use spp_core::mentions::MentionDirectory;

pub(crate) type MentionEntry = (String, String, bool);

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Directory {
    pub users: Vec<(String, String)>,
    pub teams: Vec<String>,
}

impl Directory {
    #[must_use]
    pub(crate) fn from_core(dir: &MentionDirectory) -> Self {
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

/// Snap a byte offset down to the nearest `char_boundary` of `value`.
///
/// A caret captured from a previous draft state can point into the middle of a
/// multi-byte char after an out-of-band body replacement (saved-reply insert,
/// AI draft insert, AI transform); slicing at such an offset would panic.
/// Snapping to the floor boundary keeps every downstream slice total.
#[must_use]
pub(crate) fn floor_char_boundary(value: &str, mut idx: usize) -> usize {
    if idx >= value.len() {
        return value.len();
    }
    while idx > 0 && !value.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

pub(crate) fn caret_query(value: &str, caret: usize) -> Option<(String, usize)> {
    let upto = &value[..floor_char_boundary(value, caret)];
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

#[must_use]
pub(crate) fn filter_matches(dir: &Directory, query: &str) -> Vec<MentionEntry> {
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

#[must_use]
pub(crate) fn apply_completion(value: &str, start: usize, token: &str, caret: usize) -> String {
    let start = floor_char_boundary(value, start.min(value.len()));
    let caret = floor_char_boundary(value, caret.min(value.len()));
    let after = &value[caret..];
    let sep = if after.is_empty() || after.starts_with(' ') {
        ""
    } else {
        " "
    };
    format!("{}@{}{}{}", &value[..start], token, sep, after)
}

#[must_use]
pub(crate) fn char_index_to_byte(value: &str, char_index: usize) -> usize {
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
        // inline of the deleted `Directory::from_rows` (test-only constructor)
        fn default_token(display: &str) -> String {
            display
                .split(' ')
                .next()
                .unwrap_or(display)
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
                .collect()
        }
        let mut entries: Vec<(String, String)> = [
            ("Ada Lovelace".to_string(), Some("ada".to_string())),
            ("Grace Hopper".to_string(), None),
            ("ΚIMG".to_string(), None),
        ]
        .into_iter()
        .map(|(display, mention)| {
            let token = mention.unwrap_or_else(|| default_token(&display));
            (format!("{display} (@{token})"), token)
        })
        .collect();
        entries.sort();
        entries.dedup();
        Directory {
            users: entries,
            teams: vec!["oncall".into(), "tier2".into()],
        }
    }

    #[test]
    fn caret_query_finds_token_after_at() {
        assert_eq!(caret_query("ping @ad", 9), Some(("ad".into(), 5)));
        assert_eq!(caret_query("@", 1), Some(("".into(), 0)));
        assert_eq!(caret_query("mail a@x", 8), None);
        assert_eq!(caret_query("@ada more", 4), Some(("ada".into(), 0)));
        assert_eq!(caret_query("@ada more", 5), None);
        assert_eq!(caret_query("@ada", 999), Some(("ada".into(), 0)));
    }

    #[test]
    fn filter_matches_prefix_and_contains() {
        let d = dir();
        let matches = filter_matches(&d, "ad");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, "ada");
        let matches = filter_matches(&d, "hopp");
        assert!(matches.iter().any(|m| m.0 == "Grace"));
        assert!(filter_matches(&d, "on").iter().any(|m| m.2));
        assert!(filter_matches(&d, "a b").is_empty());
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
        assert_eq!(char_index_to_byte("Κ@", 1), 2);
        assert_eq!(char_index_to_byte("Κ@", 2), 3);
        assert_eq!(char_index_to_byte("Κ@", 99), 3);
    }

    #[test]
    fn end_to_end_completion_flow() {
        let value = "can @gr take a look";
        let caret = 7;
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
