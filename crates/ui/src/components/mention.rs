use leptos::*;

#[derive(Clone, Debug, PartialEq)]
pub struct DirectoryUser {
    pub user_local_id: i64,
    pub display_name: String,
    pub mention: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DirectoryTeam {
    pub team_local_id: i64,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MentionDirectory {
    pub users: Vec<DirectoryUser>,
    pub teams: Vec<DirectoryTeam>,
}

impl MentionDirectory {
    pub fn entries(&self) -> Vec<(String, String, bool)> {
        let mut out = Vec::new();
        for u in &self.users {
            let token = match &u.mention {
                Some(m) => m.clone(),
                None => u
                    .display_name
                    .split(' ')
                    .next()
                    .unwrap_or(&u.display_name)
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
                    .collect(),
            };
            let display = match &u.mention {
                Some(m) => format!("{} (@{})", u.display_name, m),
                None => u.display_name.clone(),
            };
            out.push((token, display, false));
        }
        for t in &self.teams {
            out.push((t.name.clone(), format!("{} (team)", t.name), true));
        }
        out
    }
}

#[must_use]
pub fn parse_mention_directory(v: &serde_json::Value) -> MentionDirectory {
    let users = v
        .get("users")
        .and_then(|u| u.as_array())
        .map(|rows| {
            rows.iter()
                .map(|r| DirectoryUser {
                    user_local_id: r.get("user_local_id").and_then(|x| x.as_i64()).unwrap_or(0),
                    display_name: r
                        .get("display_name")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    mention: r
                        .get("mention")
                        .and_then(|x| x.as_str())
                        .map(str::to_string),
                })
                .collect()
        })
        .unwrap_or_default();
    let teams = v
        .get("teams")
        .and_then(|t| t.as_array())
        .map(|rows| {
            rows.iter()
                .map(|r| DirectoryTeam {
                    team_local_id: r.get("team_local_id").and_then(|x| x.as_i64()).unwrap_or(0),
                    name: r
                        .get("name")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default();
    MentionDirectory { users, teams }
}

pub fn provide_mention_directory() -> RwSignal<MentionDirectory> {
    let dir = create_rw_signal(MentionDirectory::default());
    provide_context(dir);
    spawn_local(async move {
        if let Ok(v) = crate::api::get_json::<serde_json::Value>("/api/mention-directory").await {
            dir.set(parse_mention_directory(&v));
        }
    });
    dir
}

pub fn use_mention_directory() -> RwSignal<MentionDirectory> {
    use_context::<RwSignal<MentionDirectory>>().unwrap_or_else(provide_mention_directory)
}
