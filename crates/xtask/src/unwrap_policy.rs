use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Unwrap,
    Expect,
    Panic,
    Todo,
    Unimplemented,
    Unreachable,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Unwrap => "unwrap",
            Kind::Expect => "expect",
            Kind::Panic => "panic!",
            Kind::Todo => "todo!",
            Kind::Unimplemented => "unimplemented!",
            Kind::Unreachable => "unreachable!",
        }
    }
}

struct AllowedSite {
    file: &'static str,
    kind: Kind,
    text: &'static str,
    why: &'static str,
}

const ALLOWED: &[AllowedSite] = &[
    AllowedSite {
        file: "crates/core/src/interaction_signals.rs",
        kind: Kind::Expect,
        text: "RE.get_or_init(|| regex::Regex::new($pattern).expect(\"valid classification \
               regex\"))",
        why: "D-14 class-A constant-input keep: the macro_rules static regex accessor; \
              patterns are compile-time constants — the single remaining production \
              expect outside xtask",
    },
    AllowedSite {
        file: "crates/xtask/src/strip.rs",
        kind: Kind::Expect,
        text: "let stripped = String::from_utf8(out).expect(\"stripped source must stay \
               valid utf-8\");",
        why: "dev machinery (R2.4 verdict table): the input was read as UTF-8 moments \
              earlier — constant-input class",
    },
    AllowedSite {
        file: "crates/xtask/src/package.rs",
        kind: Kind::Unwrap,
        text: "for dir in [bin_dst.parent().unwrap(), &apps_dir, appdir] {",
        why: "dev machinery: staged join-paths always have a parent component",
    },
    AllowedSite {
        file: "crates/xtask/src/package.rs",
        kind: Kind::Unwrap,
        text: "fs::create_dir_all(bin_dst.parent().unwrap())?;",
        why: "dev machinery: staged join-path always has a parent component",
    },
    AllowedSite {
        file: "crates/xtask/src/bin/xtask.rs",
        kind: Kind::Unwrap,
        text: ".unwrap_or_else(|| std::env::current_dir().unwrap())",
        why: "dev machinery: the fallback of a fallback inside workspace_root — \
              current_dir failure is unrecoverable in a dev CLI",
    },
];

pub fn run(root: &Path) -> Result<()> {
    let mut files = Vec::new();
    for crate_dir in sorted_subdirs(&root.join("crates")) {
        let src = crate_dir.join("src");
        if src.is_dir() {
            collect_rs_files(&src, &mut files);
        }
    }
    files.sort();
    let mut externals: HashSet<String> = HashSet::new();
    let mut scanned: Vec<FileData> = Vec::new();
    for path in &files {
        let Ok(bytes) = fs::read(path) else {
            continue;
        };
        let rel = rel_display(path, root);
        let raw = String::from_utf8_lossy(&bytes).into_owned();
        let analysis = strip_for_analysis(&raw);
        let raw_lines = split_lines(&raw);
        let analysis_lines = split_lines(&analysis);
        let (regions, file_externals) =
            detect_regions(&analysis_lines, path.parent().unwrap_or(path.as_path()));
        for ext in &file_externals {
            if let Some(rel_ext) = rel_opt(ext, root) {
                externals.insert(rel_ext);
            }
        }
        scanned.push(FileData {
            rel,
            raw_lines,
            analysis_lines,
            regions,
        });
    }
    let mut findings = PolicyFindings::default();
    for data in &scanned {
        let is_machinery = data.rel.starts_with("crates/xtask/");
        let whole_file_test = externals.contains(&data.rel);
        for (idx, line) in data.analysis_lines.iter().enumerate() {
            for kind in line_kinds(line) {
                if whole_file_test || in_region(&data.regions, idx) {
                    findings.test_region += 1;
                    continue;
                }
                if is_machinery {
                    findings.machinery.push(Finding {
                        rel: data.rel.clone(),
                        lineno: idx + 1,
                        kind,
                        text: data.raw_lines[idx].trim().to_string(),
                    });
                } else {
                    findings.production.push(Finding {
                        rel: data.rel.clone(),
                        lineno: idx + 1,
                        kind,
                        text: data.raw_lines[idx].trim().to_string(),
                    });
                }
            }
        }
    }
    enforce(findings, externals.len())
}

struct Finding {
    rel: String,
    lineno: usize,
    kind: Kind,
    text: String,
}

#[derive(Default)]
struct PolicyFindings {
    production: Vec<Finding>,
    machinery: Vec<Finding>,
    test_region: usize,
}

fn enforce(mut findings: PolicyFindings, external_count: usize) -> Result<()> {
    let mut violations = Vec::new();
    let mut consumed: Vec<bool> = vec![false; ALLOWED.len()];
    for bucket in [&mut findings.production, &mut findings.machinery] {
        for f in bucket.iter_mut() {
            if matches!(
                f.kind,
                Kind::Panic | Kind::Todo | Kind::Unimplemented | Kind::Unreachable
            ) {
                let scope = if f.rel.starts_with("crates/xtask/") {
                    "machinery"
                } else {
                    "production"
                };
                violations.push(format!(
                    "panic-family in {scope} code: {}:{}: {}: {}",
                    f.rel,
                    f.lineno,
                    f.kind.label(),
                    preview(&f.text)
                ));
                continue;
            }
            let mut matched = false;
            for (i, entry) in ALLOWED.iter().enumerate() {
                if !consumed[i]
                    && entry.kind == f.kind
                    && entry.file == f.rel
                    && entry.text == f.text.as_str()
                {
                    consumed[i] = true;
                    matched = true;
                    break;
                }
            }
            if !matched {
                let scope = if f.rel.starts_with("crates/xtask/") {
                    "machinery"
                } else {
                    "production"
                };
                violations.push(format!(
                    "new {} .{}: {}:{}: {}",
                    scope,
                    f.kind.label(),
                    f.rel,
                    f.lineno,
                    preview(&f.text)
                ));
            }
        }
    }
    for (i, entry) in ALLOWED.iter().enumerate() {
        if !consumed[i] {
            violations.push(format!(
                "stale allowlist entry (site no longer present): {} {}: {} — {}",
                entry.file,
                entry.kind.label(),
                preview(entry.text),
                entry.why
            ));
        }
    }
    if !violations.is_empty() {
        eprintln!("unwrap-policy: FAIL");
        for v in &violations {
            eprintln!("  {v}");
        }
        eprintln!(
            "\nThe post-R7.4 policy: zero production unwraps and panic-family;\n\
             every .expect() must be a justified allowlist entry. New site? Convert\n\
             to Result propagation, or add it to the ALLOWED table in\n\
             crates/xtask/src/unwrap_policy.rs WITH a written justification."
        );
        bail!("unwrap-policy: {} violation(s) found", violations.len());
    }
    let prod_expect = findings
        .production
        .iter()
        .filter(|f| f.kind == Kind::Expect)
        .count();
    let machinery_unwrap = findings
        .machinery
        .iter()
        .filter(|f| f.kind == Kind::Unwrap)
        .count();
    let machinery_expect = findings
        .machinery
        .iter()
        .filter(|f| f.kind == Kind::Expect)
        .count();
    println!(
        "unwrap-policy: clean (production unwrap 0 / expect {} allowed / panic-family 0; \
         machinery unwrap {} + expect {} allowed; {} test-region occurrences and {} \
         whole-file cfg(test) modules exempt)",
        prod_expect, machinery_unwrap, machinery_expect, findings.test_region, external_count
    );
    Ok(())
}

struct FileData {
    rel: String,
    raw_lines: Vec<String>,
    analysis_lines: Vec<String>,
    regions: Vec<(usize, usize)>,
}

fn in_region(regions: &[(usize, usize)], idx: usize) -> bool {
    regions.iter().any(|&(a, b)| a <= idx && idx <= b)
}

fn strip_for_analysis(src: &str) -> String {
    let text = strip_strings(src);
    blank_line_comments(&text)
}

fn strip_strings(src: &str) -> String {
    let b = src.as_bytes();
    let n = b.len();
    let mut out = String::with_capacity(n);
    let mut i = 0usize;
    let mut in_normal = false;
    let mut raw_hashes = 0usize;
    while i < n {
        let c = b[i] as char;
        if c == '\n' {
            out.push('\n');
            i += 1;
            continue;
        }
        if in_normal {
            if c == '\\' {
                if i + 1 < n && b[i + 1] == b'\n' {
                    out.push(' ');
                    out.push('\n');
                    i += 2;
                    continue;
                }
                i += 2;
                continue;
            }
            if c == '"' {
                in_normal = false;
            }
            i += 1;
            continue;
        }
        if raw_hashes > 0 {
            if c == '"'
                && i + raw_hashes < n
                && b[i + 1..i + 1 + raw_hashes].iter().all(|&x| x == b'#')
            {
                i += raw_hashes + 1;
                raw_hashes = 0;
                continue;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            let mut h = 0usize;
            let mut j = i.wrapping_sub(1);
            while j != usize::MAX && b[j] == b'#' {
                h += 1;
                j -= 1;
            }
            let is_raw = j != usize::MAX && b[j] == b'r';
            if is_raw && h > 0 {
                raw_hashes = h;
                i += 1;
                continue;
            }
            if is_raw {
                let mut k = i + 1;
                while k < n && b[k] != b'"' {
                    if b[k] == b'\n' {
                        out.push('\n');
                    } else {
                        out.push(' ');
                    }
                    k += 1;
                }
                i = k + 1;
                continue;
            }
            in_normal = true;
            i += 1;
            continue;
        }
        if c == '\'' {
            if i + 1 < n && b[i + 1] == b'\\' {
                let mut k = i + 1;
                while k < n {
                    if b[k] == b'\\' {
                        k += 2;
                        continue;
                    }
                    if b[k] == b'\'' {
                        break;
                    }
                    k += 1;
                }
                out.push_str("' '");
                i = k + 1;
                continue;
            }
            if i + 2 < n && b[i + 2] == b'\'' && b[i + 1] != b'\'' && b[i + 1] != b'\\' {
                out.push_str("' '");
                i += 3;
                continue;
            }
            out.push(c);
            i += 1;
            continue;
        }
        let ch = src[i..].chars().next().unwrap_or(c);
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn blank_line_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split('\n') {
        if let Some(col) = line.find("//") {
            out.push_str(&line[..col]);
            out.push_str(&" ".repeat(line.len() - col));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out.pop();
    out
}

fn split_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    if lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines
}

fn detect_regions(lines: &[String], dir: &Path) -> (Vec<(usize, usize)>, Vec<PathBuf>) {
    let mut regions: Vec<(usize, usize)> = Vec::new();
    let mut externals: Vec<PathBuf> = Vec::new();
    let n = lines.len();
    let mut i = 0usize;
    while i < n {
        if !is_test_attr(&lines[i]) {
            i += 1;
            continue;
        }
        let start = i;
        let line = &lines[i];
        if let Some(rest) = attr_remainder(line)
            && let Some(name) = mod_decl_name(rest)
        {
            if let Some(f) = resolve_mod_file(dir, name) {
                externals.push(f);
            }
            regions.push((start, start));
            i = start + 1;
            continue;
        }
        let mut j = i + 1;
        let mut end = n.saturating_sub(1);
        let mut closed = false;
        if let Some(col) = line.find('{') {
            end = match_brace(lines, i, col).unwrap_or(n.saturating_sub(1));
            closed = true;
        }
        if !closed {
            while j < n {
                let l = lines[j].trim();
                if l.starts_with("#[") || l.is_empty() {
                    j += 1;
                    continue;
                }
                if let Some(name) = mod_decl_name(l) {
                    if let Some(f) = resolve_mod_file(dir, name) {
                        externals.push(f);
                    }
                    end = j;
                    closed = true;
                    break;
                }
                if let Some(col) = lines[j].find('{') {
                    end = match_brace(lines, j, col).unwrap_or(n.saturating_sub(1));
                    closed = true;
                    break;
                }
                if lines[j].contains(';') {
                    end = j;
                    closed = true;
                    break;
                }
                j += 1;
            }
        }
        if !closed {
            end = n.saturating_sub(1);
        }
        regions.push((start, end));
        i = end + 1;
    }
    (regions, externals)
}

fn is_test_attr(line: &str) -> bool {
    let t = line.trim();
    t == "#[cfg(test)]"
        || t.starts_with("#[cfg(test)] ")
        || t == "#[test]"
        || t.starts_with("#[test] ")
        || t == "#[tokio::test]"
        || t.starts_with("#[tokio::test] ")
        || t == "#[rstest]"
        || t.starts_with("#[rstest] ")
}

fn attr_remainder(line: &str) -> Option<&str> {
    let t = line.trim_start();
    for prefix in ["#[cfg(test)]", "#[test]", "#[tokio::test]", "#[rstest]"] {
        if let Some(rest) = t.strip_prefix(prefix)
            && (rest.is_empty() || rest.starts_with(' '))
        {
            return Some(rest);
        }
    }
    None
}

fn mod_decl_name(line: &str) -> Option<&str> {
    let t = line.trim();
    let t = t.strip_prefix("pub(crate) ").unwrap_or(t);
    let t = t.strip_prefix("pub ").unwrap_or(t);
    let rest = t.strip_prefix("mod ")?;
    let name = rest.strip_suffix(';')?.trim_end();
    if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Some(name)
    } else {
        None
    }
}

fn resolve_mod_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let p1 = dir.join(format!("{name}.rs"));
    if p1.is_file() {
        return Some(p1);
    }
    let p2 = dir.join(name).join("mod.rs");
    if p2.is_file() {
        return Some(p2);
    }
    None
}

fn match_brace(lines: &[String], open_line: usize, open_col: usize) -> Option<usize> {
    let mut depth = 0i64;
    for (li, line) in lines.iter().enumerate().skip(open_line) {
        let seg: &str = if li == open_line {
            line.get(open_col..).unwrap_or("")
        } else {
            line.as_str()
        };
        for c in seg.chars() {
            if c == '{' {
                depth += 1;
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    return Some(li);
                }
            }
        }
    }
    None
}

fn line_kinds(line: &str) -> Vec<Kind> {
    let mut kinds = Vec::new();
    if line.contains(".unwrap()") {
        kinds.push(Kind::Unwrap);
    }
    if line.contains(".expect(") {
        kinds.push(Kind::Expect);
    }
    for (needle, kind) in [
        ("panic!(", Kind::Panic),
        ("todo!(", Kind::Todo),
        ("unimplemented!(", Kind::Unimplemented),
        ("unreachable!(", Kind::Unreachable),
    ] {
        if ident_bound(line, needle) {
            kinds.push(kind);
        }
    }
    kinds
}

fn ident_bound(line: &str, needle: &str) -> bool {
    let mut from = 0usize;
    while let Some(pos) = line[from..].find(needle) {
        let abs = from + pos;
        let before_ok = abs == 0
            || !line[..abs]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        if before_ok {
            return true;
        }
        from = abs + 1;
    }
    false
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
    rel_opt(path, root).unwrap_or_else(|| path.display().to_string())
}

fn rel_opt(path: &Path, root: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(|p| p.display().to_string())
}

fn preview(trimmed: &str) -> String {
    trimmed.chars().take(120).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<Kind> {
        line_kinds(&strip_for_analysis(src))
    }

    fn regions(src: &str) -> Vec<(usize, usize)> {
        let analysis = strip_for_analysis(src);
        detect_regions(&split_lines(&analysis), Path::new("/nonexistent")).0
    }

    fn prod_of(src: &str) -> Vec<Kind> {
        let analysis = strip_for_analysis(src);
        let lines = split_lines(&analysis);
        let (regs, _) = detect_regions(&lines, Path::new("/nonexistent"));
        let mut out = Vec::new();
        for (idx, line) in lines.iter().enumerate() {
            if in_region(&regs, idx) {
                continue;
            }
            for k in line_kinds(line) {
                out.push(k);
            }
        }
        out
    }

    #[test]
    fn unwrap_and_expect_in_code_are_detected() {
        let k = kinds("let v = lock().unwrap(); let w = x().expect(\"m\");");
        assert_eq!(k.len(), 2);
        assert!(k.contains(&Kind::Unwrap));
        assert!(k.contains(&Kind::Expect));
    }

    #[test]
    fn markers_inside_string_literals_are_not_detected() {
        let k = kinds("let s = \"calls .unwrap() here\"; let t = \"see .expect( docs\";");
        assert!(k.is_empty());
    }

    #[test]
    fn markers_inside_line_comments_are_not_detected() {
        let k = kinds("let a = 1; // historically .unwrap() lived here");
        assert!(k.is_empty());
    }

    #[test]
    fn raw_string_contents_are_invisible() {
        let k = kinds("let s = r#\"text with .unwrap() and braces {{{\"#;");
        assert!(k.is_empty());
    }

    #[test]
    fn braces_inside_strings_do_not_corrupt_regions() {
        let src = "fn f() { let s = \"}\"; g(); }\n#[cfg(test)]\nmod t {\n    fn x() { p().unwrap(); }\n}\n";
        assert_eq!(prod_of(src), Vec::<Kind>::new());
    }

    #[test]
    fn cfg_test_attr_on_multiline_fn_signature_covers_the_body() {
        let src = "#[cfg(test)]\nfn helper(\n    conn: &Connection,\n) -> Value {\n    row().expect(\"facts json\")\n}\nfn prod() {}\n";
        assert!(prod_of(src).is_empty());
        let regs = regions(src);
        assert_eq!(regs[0], (0, 5));
    }

    #[test]
    fn cfg_test_attr_on_match_arm_covers_the_arm() {
        let src = "match kind {\n    #[cfg(test)]\n    \"__test_panic\" => {\n        panic!(\"intentional\");\n    }\n    _ => prod().unwrap(),\n}\n";
        let found = prod_of(src);
        assert_eq!(found.len(), 1);
        assert!(matches!(found[0], Kind::Unwrap));
    }

    #[test]
    fn cfg_test_attr_on_same_line_item_covers_the_item() {
        let src =
            "#[cfg(test)] mod helper { fn x() { y().unwrap(); } }\nfn p() { q().unwrap(); }\n";
        let found = prod_of(src);
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn plain_test_attr_fn_is_a_test_region() {
        let src = "#[test]\nfn checks() {\n    value().unwrap();\n}\n";
        assert!(prod_of(src).is_empty());
    }

    #[test]
    fn braceless_external_mod_decl_resolves_the_sibling_file() {
        let dir = std::env::temp_dir().join("unwrap_policy_test_mod");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("sibling_tests.rs"), "fn x() { y().unwrap(); }\n").unwrap();
        let analysis = "#[cfg(test)]\nmod sibling_tests;\nfn prod() {}\n";
        let (_, ext) = detect_regions(&split_lines(analysis), &dir);
        assert_eq!(ext.len(), 1);
        assert!(ext[0].ends_with("sibling_tests.rs"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn lifetime_ticks_and_char_literals_survive_analysis() {
        let k = kinds("fn f<'a>(x: &'a str) { let c = 'x'; let b = '\\u{7b}'; }");
        assert!(k.is_empty());
    }

    #[test]
    fn panic_family_is_detected_with_identifier_boundaries() {
        assert!(ident_bound("panic!(\"x\")", "panic!("));
        assert!(!ident_bound("my_panic!(\"x\")", "panic!("));
        assert!(ident_bound("unreachable!()", "unreachable!("));
    }

    #[test]
    fn allowlist_site_texts_match_the_live_tree_lines() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..");
        for entry in ALLOWED {
            let Ok(text) = fs::read_to_string(root.join(entry.file)) else {
                panic!("allowlist file missing: {}", entry.file);
            };
            let lines = split_lines(&text);
            let found = lines.iter().any(|l| l.trim().contains(entry.text.trim()));
            assert!(
                found,
                "{} no longer matches an allowlist entry: {}",
                entry.file,
                entry.text.trim()
            );
        }
    }
}
