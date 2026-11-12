use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const SRC_DIR: &str = "src";
const TOKEN_FACTORY_PREFIX: &str = "src/design/";

const COLOR_PATTERNS: [&str; 7] = [
    "Color32::from_rgb(",
    "Color32::from_gray(",
    "Color32::from_rgba(",
    "Color32::from_black_alpha(",
    "Color32::from_white_alpha(",
    "Color32::BLACK",
    "Color32::WHITE",
];

const SPACING_ALLOWLIST: [i64; 17] = [2, 4, 5, 6, 7, 8, 10, 11, 12, 13, 14, 16, 18, 20, 22, 24, 32];

struct Finding(String, usize, String);

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn test_region_lines(lines: &[&str]) -> Vec<bool> {
    let mut skip = vec![false; lines.len()];
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim_start();
        if trimmed.starts_with("#[cfg(test)]") {
            let mut j = i + 1;
            while j < lines.len() && j <= i + 4 {
                if lines[j].contains("mod ") && lines[j].contains('{') {
                    let mut depth = 0i64;
                    let mut k = j;
                    let mut opened = false;
                    while k < lines.len() {
                        for ch in lines[k].chars() {
                            match ch {
                                '{' => {
                                    depth += 1;
                                    opened = true;
                                }
                                '}' => depth -= 1,
                                _ => {}
                            }
                        }
                        if opened && depth <= 0 {
                            break;
                        }
                        k += 1;
                    }
                    for slot in skip.iter_mut().take(k + 1).skip(i) {
                        *slot = true;
                    }
                    i = k;
                    break;
                }
                j += 1;
            }
        }
        i += 1;
    }
    skip
}

fn lint_colors(_file: &Path, rel: &str, lines: &[&str], violations: &mut Vec<Finding>) {
    if rel.replace('\\', "/").starts_with(TOKEN_FACTORY_PREFIX) {
        return;
    }
    let skip = test_region_lines(lines);
    for (idx, line) in lines.iter().enumerate() {
        if skip[idx] || line.contains("style-lint:allow") {
            continue;
        }
        for pat in COLOR_PATTERNS {
            if line.contains(pat) {
                violations.push(Finding(
                    rel.to_string(),
                    idx + 1,
                    format!("raw color construction `{pat}`"),
                ));
                break;
            }
        }
    }
}

fn numeric_arg(line: &str, marker: &str) -> Vec<i64> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(pos) = rest.find(marker) {
        let after = &rest[pos + marker.len()..];
        let num: String = after
            .chars()
            .skip_while(|c| c.is_whitespace())
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        let head = num.split('.').next().unwrap_or("");
        if let Ok(v) = head.parse::<i64>() {
            out.push(v);
        }
        rest = &rest[pos + marker.len()..];
    }
    out
}

fn lint_page_margins(file: &Path, rel: &str, lines: &[&str], violations: &mut Vec<Finding>) {
    if !rel.replace('\\', "/").starts_with("src/pages/") {
        return;
    }
    let skip = test_region_lines(lines);
    for (idx, line) in lines.iter().enumerate() {
        if skip[idx] || line.contains("style-lint:allow") {
            continue;
        }
        if !line.contains("Margin") {
            continue;
        }
        if line.chars().any(|c| c.is_ascii_digit() && c != '0') && !line.contains("scale::space") {
            violations.push(Finding(
                rel.to_string(),
                idx + 1,
                format!(
                    "numeric Margin literal in pages/ — use design::scale::space tokens: {line}"
                ),
            ));
        }
        let _ = file;
    }
}

fn lint_spacing(_file: &Path, rel: &str, lines: &[&str], violations: &mut Vec<Finding>) {
    for (idx, line) in lines.iter().enumerate() {
        if line.contains("style-lint:allow") {
            continue;
        }
        let mut literals = numeric_arg(line, "add_space(");
        literals.extend(numeric_arg(line, "Margin::same("));
        if let Some(after) = line.split("Margin::symmetric(").nth(1) {
            let arg_list = after.split(')').next().unwrap_or("");
            for part in arg_list.split(',') {
                let head = part.trim().split('.').next().unwrap_or("");
                if let Ok(v) = head.parse::<i64>() {
                    literals.push(v);
                }
            }
        }
        for v in literals {
            if v != 0 && !SPACING_ALLOWLIST.contains(&v) {
                violations.push(Finding(
                    rel.to_string(),
                    idx + 1,
                    format!("off-scale spacing literal `{v}` (DC §2.4)"),
                ));
            }
        }
    }
}

fn run_lint() -> Result<(), BTreeMap<String, Vec<String>>> {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let src = Path::new(manifest).join(SRC_DIR);
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    assert!(
        files.len() >= 20,
        "style lint found only {} source files — walker broken?",
        files.len()
    );

    let mut violations: Vec<Finding> = Vec::new();
    for path in &files {
        let rel = path
            .strip_prefix(Path::new(manifest))
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        let text = fs::read_to_string(path).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        lint_colors(path, &rel, &lines, &mut violations);
        lint_spacing(path, &rel, &lines, &mut violations);
        lint_page_margins(path, &rel, &lines, &mut violations);
    }

    if violations.is_empty() {
        Ok(())
    } else {
        let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for Finding(f, line, what) in violations {
            grouped
                .entry(f)
                .or_default()
                .push(format!("line {line}: {what}"));
        }
        Err(grouped)
    }
}

#[test]
fn no_raw_colors_and_no_off_scale_spacing() {
    match run_lint() {
        Ok(()) => {}
        Err(grouped) => {
            let mut msg = String::from("style lint violations (DC v1.0 conformance):\n");
            for (file, items) in &grouped {
                msg.push_str(&format!("  {file}:\n"));
                for item in items {
                    msg.push_str(&format!("    {item}\n"));
                }
            }
            msg.push_str("\nUse design:: tokens / design::scale instead; opt-outs need a `// style-lint:allow` justification.");
            panic!("{msg}");
        }
    }
}

#[test]
fn lint_detects_known_bad_patterns() {
    let lines = [
        "let c = egui::Color32::from_rgb(1, 2, 3);",
        "ui.add_space(9.0);",
        "egui::Margin::same(15)",
        "egui::Margin::symmetric(21, 27)",
        "let ok = egui::Color32::TRANSPARENT;",
        "ui.add_space(12.0);",
    ];
    let mut violations = Vec::new();
    lint_colors(Path::new("x.rs"), "pages/x.rs", &lines, &mut violations);
    assert_eq!(violations.len(), 1, "from_rgb must be flagged");
    violations.clear();
    lint_spacing(Path::new("x.rs"), "pages/x.rs", &lines, &mut violations);
    assert_eq!(
        violations.len(),
        4,
        "9 / 15 / 21 / 27 must be flagged (symmetric yields two)"
    );
}
