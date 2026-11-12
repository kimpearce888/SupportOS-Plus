use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

const SKIP_DIRS: [&str; 2] = ["target", ".git"];

#[derive(Default)]
pub struct StripStats {
    pub files_scanned: usize,
    pub files_with_comments: usize,
    pub files_rewritten: usize,
    pub whole_lines_removed: usize,
    pub inline_comments_removed: usize,
}

#[derive(Default)]
pub struct SourceStats {
    pub whole_lines: usize,
    pub inline_comments: usize,
}

pub struct StripOptions {
    pub workspace_root: PathBuf,
    pub roots: Vec<String>,
    pub check: bool,
}

pub fn run(opts: StripOptions) -> Result<StripStats> {
    let roots = if opts.roots.is_empty() {
        vec!["crates".to_string()]
    } else {
        opts.roots
    };
    let mut files = Vec::new();
    for root in &roots {
        collect_rs_files(&opts.workspace_root.join(root), &mut files)?;
    }
    files.sort();
    let mut stats = StripStats::default();
    for path in &files {
        let src =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let (stripped, file_stats) = strip_source(&src);
        stats.files_scanned += 1;
        if file_stats.whole_lines > 0 || file_stats.inline_comments > 0 {
            stats.files_with_comments += 1;
            stats.whole_lines_removed += file_stats.whole_lines;
            stats.inline_comments_removed += file_stats.inline_comments;
            if opts.check {
                println!("strip-comments: {}", path.display());
            } else {
                fs::write(path, &stripped)
                    .with_context(|| format!("writing {}", path.display()))?;
                stats.files_rewritten += 1;
            }
        }
    }
    if opts.check && stats.files_with_comments > 0 {
        bail!(
            "strip-comments: {} of {} scanned file(s) still contain comments",
            stats.files_with_comments,
            stats.files_scanned
        );
    }
    Ok(stats)
}

fn collect_rs_files(path: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if path.is_file() {
        if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path.to_path_buf());
        }
        return Ok(());
    }
    let entries = fs::read_dir(path).with_context(|| format!("scanning {}", path.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("scanning {}", path.display()))?;
        let p = entry.path();
        if p.is_dir() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !SKIP_DIRS.contains(&name.as_str()) {
                collect_rs_files(&p, out)?;
            }
        } else if entry.file_name().to_string_lossy().ends_with(".rs") {
            out.push(p);
        }
    }
    Ok(())
}

fn is_ws(c: u8) -> bool {
    c == b' ' || c == b'\t' || c == b'\r'
}

fn scan_block_comment(b: &[u8], start: usize) -> usize {
    let mut i = start;
    let mut depth = 0usize;
    while i < b.len() {
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            depth += 1;
            i += 2;
        } else if depth > 0 && b[i] == b'*' && i + 1 < b.len() && b[i + 1] == b'/' {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return i;
            }
        } else {
            i += 1;
        }
    }
    b.len()
}

fn trim_trailing_ws(out: &mut Vec<u8>) {
    while matches!(out.last(), Some(&b' ') | Some(&b'\t')) {
        out.pop();
    }
}

pub fn strip_source(src: &str) -> (String, SourceStats) {
    let b = src.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut stats = SourceStats::default();
    let mut i = 0usize;
    let mut line_start_out = 0usize;
    let shebang = b.len() >= 3 && b[0] == b'#' && b[1] == b'!' && b[2] != b'[';
    if shebang {
        while i < b.len() && b[i] != b'\n' {
            out.push(b[i]);
            i += 1;
        }
    }
    while i < b.len() {
        let c = b[i];
        match c {
            b'"' => {
                out.push(c);
                i += 1;
                while i < b.len() {
                    let sc = b[i];
                    out.push(sc);
                    i += 1;
                    if sc == b'\\' {
                        if i < b.len() {
                            out.push(b[i]);
                            i += 1;
                        }
                    } else if sc == b'"' {
                        break;
                    }
                }
            }
            b'r' => {
                let mut j = i + 1;
                let mut hashes = 0usize;
                while j < b.len() && b[j] == b'#' {
                    hashes += 1;
                    j += 1;
                }
                if j < b.len() && b[j] == b'"' {
                    out.extend_from_slice(&b[i..=j]);
                    i = j + 1;
                    let mut closed = false;
                    while i < b.len() && !closed {
                        let rc = b[i];
                        out.push(rc);
                        i += 1;
                        if rc == b'"' {
                            let mut k = 0usize;
                            while k < hashes && i + k < b.len() && b[i + k] == b'#' {
                                k += 1;
                            }
                            if k == hashes {
                                out.extend_from_slice(&b[i..i + k]);
                                i += k;
                                closed = true;
                            }
                        }
                    }
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            b'\'' => {
                let is_char = if i + 1 < b.len() && b[i + 1] == b'\\' {
                    true
                } else if i + 1 < b.len() {
                    src[i + 1..]
                        .chars()
                        .next()
                        .map(|ch| {
                            let after = i + 1 + ch.len_utf8();
                            after < b.len() && b[after] == b'\''
                        })
                        .unwrap_or(false)
                } else {
                    false
                };
                if is_char {
                    out.push(c);
                    i += 1;
                    while i < b.len() {
                        let cc = b[i];
                        out.push(cc);
                        i += 1;
                        if cc == b'\\' {
                            if i < b.len() {
                                out.push(b[i]);
                                i += 1;
                            }
                        } else if cc == b'\'' {
                            break;
                        }
                    }
                } else {
                    out.push(c);
                    i += 1;
                    while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                        out.push(b[i]);
                        i += 1;
                    }
                }
            }
            b'/' if i + 1 < b.len() && b[i + 1] == b'/' => {
                let mut e = i + 2;
                while e < b.len() && b[e] != b'\n' {
                    e += 1;
                }
                let line_only = out[line_start_out..].iter().all(|&x| is_ws(x));
                if line_only {
                    let next = if e < b.len() { e + 1 } else { e };
                    stats.whole_lines += b[i..next].iter().filter(|&&x| x == b'\n').count().max(1);
                    out.truncate(line_start_out);
                    i = next;
                    line_start_out = out.len();
                } else {
                    trim_trailing_ws(&mut out);
                    stats.inline_comments += 1;
                    i = e;
                }
            }
            b'/' if i + 1 < b.len() && b[i + 1] == b'*' => {
                let end = scan_block_comment(b, i);
                let mut nl = end;
                while nl < b.len() && b[nl] != b'\n' {
                    nl += 1;
                }
                let line_only = out[line_start_out..].iter().all(|&x| is_ws(x))
                    && b[end..nl].iter().all(|&x| is_ws(x));
                if line_only {
                    let next = if nl < b.len() { nl + 1 } else { nl };
                    stats.whole_lines += b[i..next].iter().filter(|&&x| x == b'\n').count().max(1);
                    out.truncate(line_start_out);
                    i = next;
                    line_start_out = out.len();
                } else {
                    trim_trailing_ws(&mut out);
                    stats.inline_comments += 1;
                    i = end;
                }
            }
            b'\n' => {
                out.push(c);
                i += 1;
                line_start_out = out.len();
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    let stripped = String::from_utf8(out).expect("stripped source must stay valid utf-8");
    (stripped, stats)
}

#[cfg(test)]
mod tests {
    use super::strip_source;

    fn stripped(src: &str) -> String {
        strip_source(src).0
    }

    #[test]
    fn whole_line_comment_is_removed_with_its_newline() {
        assert_eq!(stripped("fn a() {\n// note\n}\n"), "fn a() {\n}\n");
    }

    #[test]
    fn indented_whole_line_comment_is_removed() {
        assert_eq!(
            stripped("fn a() {\n    // note\n    b();\n}\n"),
            "fn a() {\n    b();\n}\n"
        );
    }

    #[test]
    fn trailing_comment_is_removed_and_whitespace_trimmed() {
        assert_eq!(stripped("let x = 1; // note\n"), "let x = 1;\n");
        assert_eq!(stripped("let x = 1;   // note\n"), "let x = 1;\n");
    }

    #[test]
    fn doc_and_inner_doc_comments_are_removed() {
        assert_eq!(stripped("/// docs\nfn a() {}\n"), "fn a() {}\n");
        assert_eq!(stripped("//! inner\n"), "");
    }

    #[test]
    fn inline_block_comment_is_removed() {
        assert_eq!(stripped("let x = 1 /* two */ + 3;\n"), "let x = 1 + 3;\n");
    }

    #[test]
    fn block_comment_in_argument_position_is_removed() {
        assert_eq!(stripped("f(/* arg */ y);\n"), "f( y);\n");
    }

    #[test]
    fn whole_line_block_comment_is_removed_with_its_newline() {
        assert_eq!(stripped("a();\n/* one */\nb();\n"), "a();\nb();\n");
    }

    #[test]
    fn multi_line_block_comment_is_removed_entirely() {
        assert_eq!(stripped("a();\n/* one\n   two */\nb();\n"), "a();\nb();\n");
    }

    #[test]
    fn nested_block_comments_are_removed_to_the_outer_close() {
        assert_eq!(
            stripped("a();\n/* outer /* inner */ still outer */\nb();\n"),
            "a();\nb();\n"
        );
    }

    #[test]
    fn block_comment_at_line_start_with_code_after_keeps_the_code() {
        assert_eq!(stripped("/* c */ let x = 1;\n"), " let x = 1;\n");
    }

    #[test]
    fn line_slashes_inside_a_string_are_preserved() {
        assert_eq!(
            stripped("let u = \"https://a.invalid/x\"; // real\n"),
            "let u = \"https://a.invalid/x\";\n"
        );
    }

    #[test]
    fn slashes_inside_a_raw_string_are_preserved() {
        assert_eq!(
            stripped("let s = r#\"a // b /* c\"#;\n"),
            "let s = r#\"a // b /* c\"#;\n"
        );
    }

    #[test]
    fn raw_string_with_multiple_hashes_is_preserved() {
        let src = "let s = r##\"x // y \"# z\"##;\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn plain_raw_string_ends_at_the_first_quote() {
        let src = "let s = r\"a // b\";\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn block_open_inside_a_string_is_preserved() {
        let src = "let s = \"/* not a comment */\";\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn escaped_quote_then_slashes_inside_a_string_are_preserved() {
        let src = "let s = \"a\\\"//b\";\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn byte_string_with_slashes_is_preserved() {
        let src = "let b = b\"x//y\";\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn slash_char_literal_is_preserved() {
        let src = "let c = '/';\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn slash_byte_char_literal_is_preserved() {
        let src = "let c = b'/';\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn escaped_char_literal_is_preserved() {
        let src = "let q = '\\'';\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn unicode_escape_char_literal_is_preserved() {
        let src = "let c = '\\u{1F600}';\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn multibyte_char_literal_is_preserved() {
        let src = "let c = '\u{65e5}';\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn lifetimes_are_not_consumed_as_char_literals() {
        let src = "fn f<'a>(x: &'a str) -> &'a str { x }\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn anonymous_lifetime_is_preserved() {
        let src = "fn f(x: &str) -> &'_ str { x }\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn division_is_not_treated_as_a_comment() {
        let src = "let q = a / b; let r = a/b;\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn slash_then_star_in_code_is_not_treated_as_a_comment_open() {
        let src = "let v = a * b / c * d;\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn comment_after_a_string_on_the_same_line_is_removed() {
        assert_eq!(stripped("let s = \"//\"; // real\n"), "let s = \"//\";\n");
    }

    #[test]
    fn inner_attribute_at_file_start_is_preserved() {
        let src = "#![deny(unsafe_code)]\nfn a() {}\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn shebang_line_is_preserved() {
        let src = "#!/usr/bin/env run-cargo-script\nfn a() {}\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn comment_at_eof_without_newline_is_removed() {
        assert_eq!(stripped("fn a() {}\n// tail"), "fn a() {}\n");
    }

    #[test]
    fn block_comment_at_eof_without_newline_is_removed() {
        assert_eq!(stripped("fn a() {}\n/* tail"), "fn a() {}\n");
    }

    #[test]
    fn empty_source_is_unchanged() {
        assert_eq!(stripped(""), "");
    }

    #[test]
    fn source_without_comments_is_unchanged() {
        let src = "fn main() {\n    let s = \"keep // this\";\n    println!(\"{s}\");\n}\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn consecutive_comment_lines_are_all_removed() {
        assert_eq!(
            stripped("a();\n// one\n// two\n// three\nb();\n"),
            "a();\nb();\n"
        );
    }

    #[test]
    fn doc_block_comment_is_removed() {
        assert_eq!(stripped("/** docs */\nfn a() {}\n"), "fn a() {}\n");
    }

    #[test]
    fn stripping_is_idempotent() {
        let fixture = "//! inner\n/// docs\nfn f<'a>(x: &'a str) {\n    let u = \"https://a.invalid/x\"; // keep url\n    let r = r#\"raw // text\"#;\n    let c = '/';\n    /* multi\n       line */ let y = 1;\n}\n";
        let once = stripped(fixture);
        let twice = stripped(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn stats_count_removed_comments() {
        let (_, stats) = strip_source("a();\n// one\n// two\nb(); /* inline */\n");
        assert_eq!(stats.whole_lines, 2);
        assert_eq!(stats.inline_comments, 1);
    }

    #[test]
    fn raw_identifier_is_not_treated_as_a_raw_string() {
        let src = "let r#type = 1; let s = \"x\";\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn identifier_r_followed_by_string_is_a_raw_string() {
        let src = "let s = r\"a\";\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn char_literal_range_is_preserved() {
        let src = "let ok = ('a'..='z').contains(&c);\n";
        assert_eq!(stripped(src), src);
    }

    #[test]
    fn attribute_with_string_is_preserved() {
        let src = "#[cfg(feature = \"x\")]\nfn a() {}\n";
        assert_eq!(stripped(src), src);
    }
}
