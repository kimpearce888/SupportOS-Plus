//! Mention autocomplete logic for the inbox composer (port of the Leptos
//! `MentionTextarea` helpers — the web crate is deleted at cutover, so the
//! pure logic lives here now).
//!
//! All offsets are **byte offsets into the UTF-8 string**, matching the
//! original web implementation. [`crate::pages::inbox`] converts egui's
//! char-index cursor to a byte offset before calling [`caret_query`].

use spp_core::mentions::MentionDirectory;

/// A directory entry the popup can offer: `(mention token, display label,
/// is_team)`.
pub type MentionEntry = (String, String, bool);

/// View model of users + teams, built directly from the local tables
/// (replaces the `/api/users` + `/api/teams` fetch the web widget made).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Directory {
    /// `(display_name, mention token)` for users.
    pub users: Vec<(String, String)>,
    /// Team names usable as mention tokens.
    pub teams: Vec<String>,
}

impl Directory {
    /// Build the directory from a core [`MentionDirectory`]
    /// (`spp_core::mentions::build_mention_directory`): mention name is the
    /// key, the display label comes from the user id it maps to.
    #[must_use]
    pub fn from_core(dir: &MentionDirectory) -> Self {
        let mut users: Vec<(String, String)> = dir
            .user_by_name
            .iter()
            .map(|(name, id)| {
                let display = dir
                    .display_by_user
                    .get(id)
                    .cloned()
                    .unwrap_or_else(|| format!("@{name}"));
                (display, name.clone())
            })
            .collect();
        users.sort();
        users.dedup();
        let mut teams: Vec<String> = dir.team_by_name.keys().cloned().collect();
        teams.sort();
        Self { users, teams }
    }

    /// Build the directory from explicit user rows (used by tests and by the
    /// profile-style fallback where the core directory maps by mention name).
    #[must_use]
    pub fn from_rows(users: Vec<(String, Option<String>)>, teams: Vec<String>) -> Self {
        let mut entries: Vec<(String, String)> = users
            .into_iter()
            .map(|(display, mention)| {
                let token = mention.unwrap_or_else(|| default_token(&display));
                (format!("{display} (@{token})"), token)
            })
            .collect();
        entries.sort();
        entries.dedup();
        Self {
            users: entries,
            teams,
        }
    }

    /// All selectable entries: users first, then teams (matches the web
    /// `MentionDirectory::entries` ordering).
    #[must_use]
    pub fn entries(&self) -> Vec<MentionEntry> {
        let mut out: Vec<MentionEntry> = self
            .users
            .iter()
            .map(|(display, token)| (token.clone(), display.clone(), false))
            .collect();
        out.extend(
            self.teams
                .iter()
                .map(|name| (name.clone(), format!("{name} (team)"), true)),
        );
        out
    }
}

fn default_token(display: &str) -> String {
    display
        .split(' ')
        .next()
        .unwrap_or(display)
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
        .collect()
}

/// If the caret sits in (or right after) an `@token`, return
/// `(query-without-@, byte index of the '@')`.
#[must_use]
pub fn caret_query(value: &str, caret: usize) -> Option<(String, usize)> {
    let upto = &value[..caret.min(value.len())];
    let mut idx = None;
    for (i, ch) in upto.char_indices().rev() {
        if ch == '@' {
            let before = upto[..i].chars().next_back();
            if before.is_none() || before.is_some_and(char::is_whitespace) {
                idx = Some(i);
            }
            break;
        }
        if ch.is_whitespace() {
            break;
        }
        if !is_token_char(ch) {
            break;
        }
    }
    let at = idx?;
    let text = upto[at + 1..].to_string();
    Some((text, at))
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-'
}

/// Entries matching the in-progress query (max 8, like the web widget).
#[must_use]
pub fn filter_matches(dir: &Directory, query: &str) -> Vec<MentionEntry> {
    let q = query.to_lowercase();
    if q.chars().any(|c| !is_token_char(c)) {
        return Vec::new();
    }
    dir.entries()
        .into_iter()
        .filter(|(token, display, _)| {
            token.to_lowercase().starts_with(&q) || display.to_lowercase().contains(&q)
        })
        .take(8)
        .collect()
}

/// Replace the `@query` at `start..caret` with `@token` plus a separating
/// space when needed.
#[must_use]
pub fn apply_completion(value: &str, start: usize, token: &str, caret: usize) -> String {
    let caret = caret.min(value.len());
    let after = &value[caret..];
    let sep = if after.is_empty() || after.starts_with(' ') {
        ""
    } else {
        " "
    };
    format!("{}@{}{}{}", &value[..start], token, sep, after)
}

/// Convert a char-index cursor (egui `CCursor.index.0`) to a byte offset.
#[must_use]
pub fn char_index_to_byte(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map(|(byte, _)| byte)
        .unwrap_or(value.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> Directory {
        Directory::from_rows(
            vec![
                ("Ada Lovelace".into(), Some("ada".into())),
                ("Grace Hopper".into(), None),
                ("ΚIMG".into(), None), // non-ASCII display name
            ],
            vec!["oncall".into(), "tier2".into()],
        )
    }

    #[test]
    fn caret_query_finds_token_after_at() {
        assert_eq!(caret_query("ping @ad", 9), Some(("ad".into(), 5)));
        assert_eq!(caret_query("@", 1), Some(("".into(), 0)));
        // '@' must start a word.
        assert_eq!(caret_query("mail a@x", 8), None);
        // Caret right after the last token char keeps the popup open.
        assert_eq!(caret_query("@ada more", 4), Some(("ada".into(), 0)));
        // A space closes the token.
        assert_eq!(caret_query("@ada more", 5), None);
        // Caret clamped beyond the end.
        assert_eq!(caret_query("@ada", 999), Some(("ada".into(), 0)));
    }

    #[test]
    fn filter_matches_prefix_and_contains() {
        let d = dir();
        let matches = filter_matches(&d, "ad");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, "ada");
        // Display-name substring also matches.
        let matches = filter_matches(&d, "hopp");
        assert!(matches.iter().any(|m| m.0 == "Grace"));
        // Teams match too.
        assert!(filter_matches(&d, "on").iter().any(|m| m.2));
        // Invalid query chars match nothing.
        assert!(filter_matches(&d, "a b").is_empty());
        // Empty query lists entries (max 8).
        assert_eq!(filter_matches(&d, "").len(), 5);
    }

    #[test]
    fn apply_completion_inserts_token_and_separator() {
        assert_eq!(apply_completion("hi @ad", 3, "ada", 6), "hi @ada");
        assert_eq!(apply_completion("hi @ad!", 3, "ada", 6), "hi @ada !");
        assert_eq!(apply_completion("@ad", 0, "ada", 3), "@ada");
    }

    #[test]
    fn char_index_to_byte_handles_multibyte() {
        assert_eq!(char_index_to_byte("Κ@", 1), 2); // Κ is 2 bytes
        assert_eq!(char_index_to_byte("Κ@", 2), 3);
        assert_eq!(char_index_to_byte("Κ@", 99), 3);
    }

    #[test]
    fn end_to_end_completion_flow() {
        let value = "can @gr take a look";
        let caret = 7; // right after the token "gr"
        let (query, start) = caret_query(value, caret).expect("token");
        assert_eq!(query, "gr");
        assert_eq!(start, 4);
        let matches = filter_matches(&dir(), &query);
        let (token, _, _) = matches
            .iter()
            .find(|(t, _, _)| t == "Grace")
            .or_else(|| matches.first())
            .cloned()
            .expect("match");
        let completed = apply_completion(value, start, &token, caret);
        assert!(completed.contains("@Grace"));
    }

    #[test]
    fn directory_from_core_maps_mentions() {
        // Build a core directory via its public fields (no foreign impl
        // needed): mention name → user id, id → display name.
        let mut core = MentionDirectory::default();
        core.user_by_name.insert("ada".into(), 1);
        core.user_by_name.insert("kim".into(), 2);
        core.display_by_user.insert(1, "Ada Lovelace".into());
        core.display_by_user.insert(2, "Kim P.".into());
        let d = Directory::from_core(&core);
        assert!(d.users.iter().any(|(_, token)| token == "ada"));
        assert!(d.users.iter().any(|(_, token)| token == "kim"));
    }
}
