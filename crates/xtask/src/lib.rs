pub mod icons;
pub mod package;

#[cfg(test)]
mod hygiene {
    //! FIND-003 regression guards.
    //!
    //! The prior QA session (`qa-audit/20261019T050847Z`) committed app-local
    //! secret-envelope key material (`evidence/artifacts/qa-profile-*/secrets.key`)
    //! as repo artifacts. On 2026-10-25 the maintainer authorized a history
    //! purge (git-filter-repo, all refs incl. release tags) and the material
    //! was removed from every commit. These tests prevent reintroduction: the
    //! key files must never reappear in the tree, and the `.gitignore` rule
    //! that blocks them must stay in place.

    use std::path::{Path, PathBuf};

    /// Directories excluded from the scan (VCS internals / build output).
    const SKIP_DIRS: &[&str] = &[".git", "target", "node_modules", "dist"];

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .canonicalize()
            .expect("repo root must resolve from the xtask crate")
    }

    fn find_files_named(dir: &Path, name: &str, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if path.is_dir() {
                if !SKIP_DIRS.contains(&file_name) {
                    find_files_named(&path, name, out);
                }
            } else if file_name == name {
                out.push(path);
            }
        }
    }

    #[test]
    fn no_secret_key_artifacts_anywhere_in_the_repo() {
        let mut hits = Vec::new();
        find_files_named(&repo_root(), "secrets.key", &mut hits);
        assert!(
            hits.is_empty(),
            "FIND-003 regression: secret-envelope key material must never be \
             tracked or left in the tree; found {hits:?}"
        );
    }

    #[test]
    fn gitignore_keeps_the_secrets_key_never_track_rule() {
        let ignore = std::fs::read_to_string(repo_root().join(".gitignore"))
            .expect(".gitignore must exist at the repo root");
        assert!(
            ignore.lines().any(|l| l.trim() == "secrets.key"),
            "FIND-003 regression: .gitignore must keep the `secrets.key` rule"
        );
    }
}
