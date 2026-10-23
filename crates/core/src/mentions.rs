use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;
use rusqlite::{Connection, params};

use crate::error::Result;

pub const MAX_BODY_BYTES: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mention {
    User { username: String },
    Team { teamname: String },
}

impl Mention {
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::User { username } => format!("@{username}"),
            Self::Team { teamname } => format!("@team:{teamname}"),
        }
    }
}

fn user_mention_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"@(\w{1,64})").expect("static regex compiles"))
}

fn team_mention_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"@team:([\w-]{1,64})").expect("static regex compiles"))
}

fn is_preceded_by_word_char(body: &str, pos: usize) -> bool {
    if pos == 0 {
        return false;
    }
    let bytes = body.as_bytes();
    let mut i = pos - 1;
    while i > 0 && (bytes[i] & 0xC0) == 0x80 {
        i -= 1;
    }
    let prev = body[i..pos].chars().next();
    match prev {
        Some(c) => c.is_alphanumeric() || c == '_',
        None => false,
    }
}

pub fn scan_for_mentions(body: &str) -> Result<Vec<Mention>> {
    if body.len() > MAX_BODY_BYTES {
        return Err(crate::error::Error::Other(
            format!(
                "mention scan refused: body {} bytes exceeds max {} bytes",
                body.len(),
                MAX_BODY_BYTES
            )
            .into(),
        ));
    }

    if !body.contains('@') {
        return Ok(Vec::new());
    }

    let mut mentions: Vec<Mention> = Vec::new();
    let mut seen_usernames: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut seen_teamnames: std::collections::HashSet<String> = std::collections::HashSet::new();

    for cap in team_mention_regex().captures_iter(body) {
        if let Some(m) = cap.get(0) {
            if is_preceded_by_word_char(body, m.start()) {
                continue;
            }
            if let Some(name_match) = cap.get(1) {
                let teamname = name_match.as_str().to_string();
                if seen_teamnames.insert(teamname.clone()) {
                    mentions.push(Mention::Team { teamname });
                }
            }
        }
    }

    for cap in user_mention_regex().captures_iter(body) {
        if let Some(m) = cap.get(0) {
            if is_preceded_by_word_char(body, m.start()) {
                continue;
            }
            if let Some(name_match) = cap.get(1) {
                let username = name_match.as_str().to_string();
                if username == "team" {
                    continue;
                }
                if seen_usernames.insert(username.clone()) {
                    mentions.push(Mention::User { username });
                }
            }
        }
    }

    Ok(mentions)
}

#[derive(Debug, Clone, Default)]
pub struct MentionDirectory {
    pub user_by_name: HashMap<String, i64>,
    pub team_by_name: HashMap<String, i64>,
    pub display_by_user: HashMap<i64, String>,
}

type UserRow = (
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

pub fn build_mention_directory(conn: &Connection) -> Result<MentionDirectory> {
    let mut dir = MentionDirectory::default();
    let users: Vec<UserRow> = {
        let mut stmt = conn.prepare(
            "SELECT id, first_name, last_name, mention, email FROM users
             WHERE deleted_at IS NULL",
        )?;

        stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (id, first, last, mention, email) in users {
        let display = [first.clone(), last.clone()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        let display = if !display.is_empty() {
            display
        } else {
            email.unwrap_or_else(|| format!("user #{id}"))
        };
        dir.display_by_user.insert(id, display);
        let mut candidates: Vec<String> = Vec::new();
        if let Some(m) = mention {
            candidates.push(m);
        }
        if let Some(f) = &first {
            candidates.push(f.clone());
        }
        let full = [first, last]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        if !full.is_empty() {
            candidates.push(full.clone());
            candidates.push(full.split_whitespace().collect::<String>());
            candidates.push(full.split_whitespace().collect::<Vec<_>>().join("."));
        }
        for c in candidates {
            let key = c.trim().to_lowercase();
            if !key.is_empty() {
                dir.user_by_name.entry(key).or_insert(id);
            }
        }
    }
    let teams: Vec<(i64, String)> = {
        let mut stmt = conn.prepare("SELECT id, name FROM teams")?;

        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (id, name) in teams {
        let key = name.trim().to_lowercase();
        if !key.is_empty() {
            dir.team_by_name.entry(key).or_insert(id);
        }
    }
    Ok(dir)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMention {
    pub token: String,
    pub user_local_id: Option<i64>,
    pub team_local_id: Option<i64>,
    pub display: String,
}

fn is_token_char(c: char) -> bool {
    // Clean-audit item 62: scan_for_mentions tokenizes on the regex `\w`
    // (Unicode), but this scanner used to accept ASCII only — a username
    // like "josé" or "见" could be recorded as a raw mention by the scan
    // side while never resolving to a notification here. The token charset
    // is now Unicode (the '.' and '-' extensions stay: the directory's
    // user keys include dotted full names like "jane.doe").
    c.is_alphanumeric() || c == '.' || c == '_' || c == '-'
}

/// The `[\w-]` run `scan_for_mentions`'s team regex consumes after the
/// `team:` prefix (Unicode word characters plus dash).
fn is_team_prefix_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

pub fn parse_mentions(body: &str, directory: &MentionDirectory) -> Result<Vec<ParsedMention>> {
    if body.len() > MAX_BODY_BYTES {
        return Err(crate::error::Error::Other(
            format!(
                "mention parse refused: body {} bytes exceeds max {} bytes",
                body.len(),
                MAX_BODY_BYTES
            )
            .into(),
        ));
    }
    let mut mentions: Vec<ParsedMention> = Vec::new();
    let mut seen_users: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let mut seen_teams: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let team_max_len = directory
        .team_by_name
        .keys()
        .map(|k| k.len())
        .max()
        .unwrap_or(0);

    let mut search_from = 0usize;
    while let Some(found) = body[search_from..].find('@') {
        let at = search_from + found;
        let after_at = at + 1;
        // Clean-audit item 62: an '@' glued to a word character is not a
        // mention — "support@jane" inside an email address used to fire a
        // mention for user "jane" here while scan_for_mentions (which
        // checks the preceding character) correctly ignored it.
        if is_preceded_by_word_char(body, at) {
            search_from = after_at;
            continue;
        }
        // Clean-audit item 62: the `@team:name` prefixed form — the ONLY
        // team form scan_for_mentions recognizes (its `@team:([\w-]{1,64})`
        // regex). This scanner used to drop it silently: the token stopped
        // at the colon and the bare-name window below never matches a
        // "team:…" candidate, so every @team: mention displayed in the
        // thread's mentions_json produced no notification.
        if body[after_at..].starts_with("team:") {
            let run_start = after_at + "team:".len();
            let run: String = body[run_start..]
                .chars()
                .take_while(|c| is_team_prefix_char(*c))
                .take(64)
                .collect();
            if !run.is_empty() {
                if let Some(tid) = directory.team_by_name.get(&run.to_lowercase())
                    && seen_teams.insert(*tid)
                {
                    mentions.push(ParsedMention {
                        token: run.clone(),
                        user_local_id: None,
                        team_local_id: Some(*tid),
                        display: run.clone(),
                    });
                }
                // Consume the whole prefixed run either way — with or
                // without a resolvable team, this was a team mention.
                search_from = run_start + run.len();
                continue;
            }
        }
        let token: String = body[after_at..]
            .chars()
            .take_while(|c| is_token_char(*c))
            .collect();
        if token.is_empty() {
            search_from = after_at;
            continue;
        }
        let lower = token.to_lowercase();
        // Clean-audit item 62: scan_for_mentions never resolves a bare
        // "@team" as a USER mention (the word belongs to the prefixed team
        // form); mirror that here.
        if lower != "team"
            && let Some(uid) = directory.user_by_name.get(&lower)
        {
            if seen_users.insert(*uid) {
                mentions.push(ParsedMention {
                    token: token.clone(),
                    user_local_id: Some(*uid),
                    team_local_id: None,
                    display: directory
                        .display_by_user
                        .get(uid)
                        .cloned()
                        .unwrap_or_else(|| token.clone()),
                });
            }
            search_from = after_at + token.len();
            continue;
        }
        if !directory.team_by_name.is_empty() {
            let window_len = 60.min(team_max_len.max(lower.len()));
            let window_end = (after_at + window_len).min(body.len());
            let window = &body[after_at..window_end];
            let mut matched: Option<(i64, usize, String)> = None;
            let mut len = window.len().min(team_max_len + 12);
            while len >= lower.len() {
                while len > 0 && !window.is_char_boundary(len) {
                    len -= 1;
                }
                if len < lower.len() {
                    break;
                }
                let raw = &window[..len];
                let candidate = raw.trim().to_lowercase();
                let candidate = candidate.split_whitespace().collect::<Vec<_>>().join(" ");
                if !candidate.is_empty()
                    && let Some(tid) = directory.team_by_name.get(&candidate)
                    && !seen_teams.contains(tid)
                {
                    let next_char = window[len..].chars().next();
                    let boundary_ok = next_char.is_none_or(|c| !is_token_char(c));
                    if boundary_ok {
                        matched = Some((*tid, len, raw.trim().to_string()));
                        break;
                    }
                }
                len -= 1;
            }
            if let Some((tid, matched_len, raw)) = matched {
                seen_teams.insert(tid);
                mentions.push(ParsedMention {
                    token: raw.clone(),
                    user_local_id: None,
                    team_local_id: Some(tid),
                    display: raw,
                });
                search_from = after_at + matched_len;
                continue;
            }
        }
        search_from = after_at + token.len();
    }
    Ok(mentions)
}

pub fn team_members(conn: &Connection, team_local_id: i64) -> Result<Vec<i64>> {
    let mut stmt = match conn.prepare("SELECT user_id FROM team_members WHERE team_id = ?1") {
        Ok(stmt) => stmt,
        Err(_) => return Ok(Vec::new()),
    };
    let rows = stmt
        .query_map(params![team_local_id], |r| r.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Clean-audit batch 25 (item 62): parse_mentions must agree with
/// scan_for_mentions on WHAT is a mention — the pre-fix parser was
/// ASCII-only (Unicode usernames never resolved) and had no left boundary
/// (emails fired mentions); it also dropped the `@team:name` prefixed form
/// the scanner records.
#[cfg(test)]
mod batch25_item62_tests {
    use super::*;

    fn directory() -> MentionDirectory {
        let mut dir = MentionDirectory::default();
        // José — a user whose name needs a non-ASCII token char.
        dir.user_by_name.insert("josé".into(), 1);
        dir.display_by_user.insert(1, "José García".into());
        // A dotted full name (the directory's derived candidate keys).
        dir.user_by_name.insert("jane.doe".into(), 2);
        dir.display_by_user.insert(2, "Jane Doe".into());
        // A user literally named "team" — the scanner never resolves a
        // bare @team as a user.
        dir.user_by_name.insert("team".into(), 3);
        dir.display_by_user.insert(3, "Team User".into());
        dir.team_by_name.insert("billing".into(), 10);
        dir.team_by_name.insert("tier-2".into(), 11);
        dir
    }

    #[test]
    fn unicode_usernames_resolve_like_the_scanner_records_them() {
        let dir = directory();
        let mentions = parse_mentions("ping @josé please", &dir).expect("parse");
        // Pre-fix: the token stopped at 'é' (ASCII-only charset), the
        // lookup for "jos" failed, and the mention the scanner recorded
        // never became a notification.
        assert_eq!(
            mentions.len(),
            1,
            "the Unicode username resolves: {mentions:?}"
        );
        assert_eq!(mentions[0].user_local_id, Some(1));
        assert_eq!(mentions[0].token, "josé");
    }

    #[test]
    fn a_mention_glued_to_a_word_character_is_not_a_mention() {
        let dir = directory();
        // Pre-fix: "support@jane.doe" fired a user mention for Jane.
        let mentions =
            parse_mentions("contact support@jane.doe or @jane.doe directly", &dir).expect("parse");
        assert_eq!(
            mentions.len(),
            1,
            "only the standalone mention fires, not the email: {mentions:?}"
        );
        assert_eq!(mentions[0].user_local_id, Some(2));
    }

    #[test]
    fn the_team_prefixed_form_resolves_to_the_team() {
        let dir = directory();
        // Pre-fix: the token stopped at ':' and the bare-name window never
        // matched a "team:…" candidate — the scanner recorded
        // "@team:billing" in mentions_json while nobody was notified.
        let mentions = parse_mentions("escalating @team:billing now", &dir).expect("parse");
        assert_eq!(
            mentions.len(),
            1,
            "the prefixed team form resolves: {mentions:?}"
        );
        assert_eq!(mentions[0].team_local_id, Some(10));
        assert_eq!(mentions[0].user_local_id, None);

        // Dashed team names ride the [\w-] run exactly like the scanner's
        // regex.
        let mentions = parse_mentions("@team:tier-2 over", &dir).expect("parse");
        assert_eq!(mentions[0].team_local_id, Some(11), "dashed name");
    }

    #[test]
    fn the_team_prefixed_form_consumes_the_whole_run() {
        let dir = directory();
        // An unresolvable team must not leak its tail into another
        // mention: "@team:nobody @josé" — after consuming "team:nobody",
        // the next '@' is José's.
        let mentions = parse_mentions("@team:nobody @josé", &dir).expect("parse");
        assert_eq!(
            mentions.len(),
            1,
            "the unknown team yields nothing and José still resolves: {mentions:?}"
        );
        assert_eq!(mentions[0].user_local_id, Some(1));
    }

    #[test]
    fn a_bare_team_word_is_not_a_user_mention() {
        let dir = directory();
        // The scanner excludes username "team"; the parser mirrors that
        // even though a user named "team" exists in the directory.
        let mentions = parse_mentions("standup @team now", &dir).expect("parse");
        assert!(
            mentions.iter().all(|m| m.user_local_id.is_none()),
            "no USER mention for the bare @team word: {mentions:?}"
        );
    }

    #[test]
    fn the_scanner_and_parser_agree_on_plain_bodies() {
        let dir = directory();
        // The same body through both scanners: everything the scanner
        // records, the parser resolves.
        let body = "hey @josé and @team:billing — email me at x@jane.doe";
        let scanned = scan_for_mentions(body).expect("scan");
        let parsed = parse_mentions(body, &dir).expect("parse");
        assert_eq!(scanned.len(), 2, "scanner: josé + team:billing");
        assert_eq!(parsed.len(), 2, "parser resolves both, ignores the email");
        assert!(parsed.iter().any(|m| m.user_local_id == Some(1)));
        assert!(parsed.iter().any(|m| m.team_local_id == Some(10)));
    }
}
