use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

const MARKERS: [&str; 3] = ["simulated", "fake", "demo"];

const MACHINERY_EXEMPT: &[&str] = &["crates/xtask/src/gate.rs"];

type AllowlistEntry = (&'static str, &'static str);

const ALLOWLIST: &[(&str, &[AllowlistEntry])] = &[(
    "crates/core/src/audit.rs",
    &[
        (
            "\"DELETE FROM application_settings WHERE key IN ('demo_mode', \
             'demo_content_purged')\",",
            "the m050 purge itself: production code whose job is deleting retired \
             demo-mode settings rows",
        ),
        (
            "\"m050: removed retired demo-mode settings rows\"",
            "log message of the same purge — it reports the removal, it does not \
             fabricate",
        ),
    ],
)];

pub fn run(root: &Path) -> Result<()> {
    let violations = scan(root);
    if violations.is_empty() {
        println!("no-simulation gate: clean");
        return Ok(());
    }
    eprintln!("no-simulation gate: FAIL");
    for v in &violations {
        eprintln!("  {v}");
    }
    eprintln!(
        "\nProduction code must not fabricate data. If the marker is an honest\n\
         historical reference or removal machinery, add it to the allowlist in\n\
         crates/xtask/src/gate.rs WITH a justification."
    );
    bail!(
        "no-simulation gate: {} violation(s) found",
        violations.len()
    );
}

fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

fn split_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    if lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines
}

fn strip_test_regions(lines: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    let n = lines.len();
    while i < n {
        let line = &lines[i];
        if !line.contains("#[cfg(test)]") || is_comment(line) {
            out.push(line.clone());
            i += 1;
            continue;
        }
        let start = i;
        let mut depth = 0i64;
        let mut started = false;
        while i < n {
            if !started && is_comment(&lines[i]) {
                i += 1;
                continue;
            }
            depth += count_char(&lines[i], '{') as i64 - count_char(&lines[i], '}') as i64;
            if lines[i].contains('{') {
                started = true;
            }
            i += 1;
            if started && depth <= 0 {
                break;
            }
            if !started {
                let prev = &lines[i - 1];
                if prev.contains(';') && format!(" {prev}").contains(" mod ") {
                    break;
                }
            }
        }
        out.extend(std::iter::repeat_n(String::new(), i - start));
    }
    out
}

fn count_char(s: &str, c: char) -> usize {
    s.chars().filter(|&x| x == c).count()
}

fn scan(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    for crate_dir in sorted_subdirs(&root.join("crates")) {
        let src = crate_dir.join("src");
        if src.is_dir() {
            collect_rs_files(&src, &mut files);
        }
    }
    files.sort();
    let mut violations = Vec::new();
    for path in files {
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let rel = rel_display(&path, root);
        if MACHINERY_EXEMPT.contains(&rel.as_str()) {
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        violations.extend(scan_text(&rel, &text));
    }
    violations
}

fn scan_text(rel: &str, text: &str) -> Vec<String> {
    let lines = strip_test_regions(&split_lines(text));
    let mut violations = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        if is_comment(line) {
            continue;
        }
        let lowered = line.to_lowercase();
        let matched: Vec<&str> = MARKERS
            .iter()
            .copied()
            .filter(|m| lowered.contains(m))
            .collect();
        if matched.is_empty() {
            continue;
        }
        if allowlist_allows(rel, line.trim()) {
            continue;
        }
        violations.push(format!(
            "{rel}:{}: marker {}: {}",
            idx + 1,
            format_markers(&matched),
            preview(line.trim())
        ));
    }
    violations
}

fn sorted_subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            out.push(p);
        }
    }
    out.sort();
    out
}

fn collect_rs_files(path: &Path, out: &mut Vec<PathBuf>) {
    if !path.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_rs_files(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(p);
        }
    }
}

fn rel_display(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn allowlist_allows(rel: &str, trimmed: &str) -> bool {
    ALLOWLIST
        .iter()
        .filter(|(file, _)| *file == rel)
        .flat_map(|(_, entries)| entries.iter())
        .any(|(line, _)| *line == trimmed)
}

fn format_markers(markers: &[&str]) -> String {
    let inner: Vec<String> = markers.iter().map(|m| format!("'{m}'")).collect();
    format!("[{}]", inner.join(", "))
}

fn preview(trimmed: &str) -> String {
    trimmed.chars().take(120).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blanked(src: &str) -> Vec<String> {
        strip_test_regions(&split_lines(src))
    }

    fn scan_for_test(rel: &str, src: &str) -> Vec<String> {
        scan_text(rel, src)
    }

    #[test]
    fn marker_in_production_line_is_a_violation() {
        let v = scan_for_test("crates/core/src/x.rs", "let name = \"demo account\";\n");
        assert_eq!(v.len(), 1);
        assert!(v[0].starts_with("crates/core/src/x.rs:1: marker ['demo']"));
    }

    #[test]
    fn marker_matching_is_case_insensitive() {
        let v = scan_for_test("crates/core/src/x.rs", "let a = Fake_Account;\n");
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("marker ['fake']"));
    }

    #[test]
    fn comment_line_markers_are_skipped() {
        let v = scan_for_test(
            "crates/core/src/x.rs",
            "// the demo fixture below is described here\nlet a = 1;\n",
        );
        assert!(v.is_empty());
    }

    #[test]
    fn cfg_test_inline_region_is_blanked_with_line_numbers_preserved() {
        let out = blanked(
            "fn prod() {}\n#[cfg(test)]\nmod tests {\n    fn f() { let d = \"demo\"; }\n}\n",
        );
        assert_eq!(out[0], "fn prod() {}");
        assert_eq!(out[1], "");
        assert_eq!(out[2], "");
        assert_eq!(out[3], "");
        assert_eq!(out[4], "");
        assert_eq!(out.len(), 5);
    }

    #[test]
    fn cfg_test_marker_inside_a_comment_is_not_an_attribute() {
        let out = blanked("// #[cfg(test)]\nlet demo = 1;\n");
        assert_eq!(out[0], "// #[cfg(test)]");
        assert_eq!(out[1], "let demo = 1;");
    }

    #[test]
    fn braceless_test_mod_declaration_ends_at_that_line() {
        let out = blanked("#[cfg(test)] mod extra;\nfn prod() {}\n");
        assert_eq!(out[0], "");
        assert_eq!(out[1], "fn prod() {}");
    }

    #[test]
    fn allowlist_entry_matches_by_exact_trimmed_content() {
        let src = "        \"DELETE FROM application_settings WHERE key IN \
                   ('demo_mode', 'demo_content_purged')\",\n";
        let v = scan_for_test("crates/core/src/audit.rs", src);
        assert!(v.is_empty());
    }

    #[test]
    fn allowlist_entry_does_not_match_in_another_file() {
        let src = "\"m050: removed retired demo-mode settings rows\"\n";
        let v = scan_for_test("crates/core/src/other.rs", src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn violation_preview_is_truncated_to_one_hundred_twenty_chars() {
        let long = format!("let s = \"demo{}\";\n", "x".repeat(200));
        let v = scan_for_test("crates/core/src/x.rs", &long);
        assert_eq!(v.len(), 1);
        assert!(v[0].len() < 200);
    }

    #[test]
    fn the_gate_module_itself_is_exempt_from_its_own_scan() {
        let v = scan_for_test("crates/xtask/src/gate.rs", "const X: &str = \"demo\";\n");
        assert_eq!(v.len(), 1);
        assert!(MACHINERY_EXEMPT.contains(&"crates/xtask/src/gate.rs"));
    }
}
