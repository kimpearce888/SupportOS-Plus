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
    c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-'
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
        let token: String = body[after_at..]
            .chars()
            .take_while(|c| is_token_char(*c))
            .collect();
        if token.is_empty() {
            search_from = after_at;
            continue;
        }
        let lower = token.to_lowercase();
        if let Some(uid) = directory.user_by_name.get(&lower) {
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
