#!/usr/bin/env python3
"""Batch 32 verification harness (clean-audit items 127-136).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact fix
landed, mirroring the batch-16..31 harness protocol.

Items:
  127  digest chip 16.max slice: dead truncation + OOB risk (reports.rs)
  128  QA rows render `#Some(42)` (reports.rs)
  129  literal `\\` mid-sentence in 3 hover texts (issue_radar.rs)
  130  notification toggle reads the wrong list for Mentions rows
       (notifications.rs)
  131  incidents: invalid ids silently dropped; filters without refetch
       (incidents.rs)
  132  diagnostics clear_log via reader lane, errors swallowed
       (diagnostics.rs)
  133  palette: same-frame Enter confirms stale hit; no outside-click
       close (palette.rs)
  134  concave polygon to convex_polygon area fill (chart.rs)
  135  stale inserted_draft provenance -> phantom AI-draft acceptances
       (inbox.rs)
  136  mention popup highlight never moves (inbox.rs)
"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SHELL = ROOT / "crates" / "app-shell" / "src"
PAGES = SHELL / "pages"
FAILURES: list[str] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    status = "OK " if ok else "FAIL"
    print(f"[{status}] {name}" + (f" — {detail}" if detail and not ok else ""))
    if not ok:
        FAILURES.append(f"{name}: {detail}")


def read(root: Path, p: str) -> str:
    return (root / p).read_text(encoding="utf-8")


def read_code(root: Path, p: str) -> str:
    """Source without comment lines — for checks whose needle may
    legitimately appear inside a comment describing the pre-fix defect."""
    return "\n".join(
        line
        for line in read(root, p).splitlines()
        if not line.lstrip().startswith("//")
    )


def function_body(src: str, needle: str, end: str = "\n}\n") -> str:
    start = src.index(needle)
    end_idx = src.index(end, start)
    return src[start:end_idx]


# ----------------------------------------------------------------- item 127
def verify_item127() -> None:
    reports = read_code(PAGES, "reports.rs")
    check(
        "127: the digest chip caps at 16 with `min` (truncate when longer)",
        "16.min(digest.created_at.len())" in reports,
    )
    check(
        "127: the inverted `16.max` slice is gone",
        "16.max(" not in reports,
    )


# ----------------------------------------------------------------- item 128
def verify_item128() -> None:
    reports = read_code(PAGES, "reports.rs")
    check(
        "128: the Option Debug format is gone",
        "#{id:?}" not in reports,
    )
    check(
        "128: a present id renders as `#42`, a missing one as `#?`",
        'Some(n) => format!("#{n}")' in reports and '"#?".to_string()' in reports,
    )


# ----------------------------------------------------------------- item 129
def verify_item129() -> None:
    raw = read(PAGES, "issue_radar.rs")
    # A line ending in two literal backslashes is an ESCAPED backslash —
    # the rendered text carried a stray `\` plus a real newline.
    doubled = [
        line
        for line in raw.splitlines()
        if line.rstrip().endswith("\\\\")
    ]
    check("129: no string line ends in an escaped `\\\\`", not doubled, str(doubled[:3]))
    for needle in (
        "issue clusters \\\n",
        "in AI Center \\\n",
        "a few seconds \\\n",
    ):
        check(
            f"129: `{needle.strip()!r}` continues the line (single backslash)",
            needle in raw,
        )


# ----------------------------------------------------------------- item 130
def verify_item130() -> None:
    notifications = read_code(PAGES, "notifications.rs")
    body = function_body(notifications, "fn toggle_read")
    check(
        "130: the toggle looks the row up in BOTH lists",
        ".chain(self.mentions.iter())" in body,
    )
    check(
        "130: the records-only lookup is gone",
        "self.records\n            .iter()\n            .find(|r| r.id == id)" not in body,
    )
    check(
        "130: the regression test ships",
        "fn mention_toggle_reads_the_mentions_list_when_records_miss_the_row" in notifications,
    )


# ----------------------------------------------------------------- item 131
def verify_item131() -> None:
    incidents = read_code(PAGES, "incidents.rs")
    body = function_body(incidents, "fn create(&mut self, db: &DbHandle)")
    check(
        "131: every typed token is kept so skips can be named",
        "s.trim().parse::<i64>().ok().filter(|v| *v > 0)," in body
        and "s.trim().to_string()," in body,
    )
    check(
        "131: nonexistent ids are collected as `missing`",
        "missing.push(id.to_string())" in body,
    )
    check(
        "131: the notice names the skipped ids and the linked count",
        'missing.join(", ")' in body and "nonexistent id(s) skipped:" in body,
    )
    ui = function_body(incidents, "pub fn ui(&mut self, ui: &mut egui::Ui, db: &DbHandle)")
    for control, prev in (
        ("self.filter_status", "prev_status"),
        ("self.filter_severity", "prev_severity"),
        ("self.filter_open_only", "prev_open_only"),
    ):
        check(
            f"131: the {control} control refetches on change",
            f"{control} != {prev}" in ui,
        )
    check(
        "131: the skipped-ids regression test ships",
        "fn create_names_the_skipped_conversation_ids" in incidents,
    )


# ----------------------------------------------------------------- item 132
def verify_item132() -> None:
    diagnostics = read_code(PAGES, "diagnostics.rs")
    body = function_body(diagnostics, "fn clear_log")
    check(
        "132: the DELETE runs on the writer lane",
        "db.with_write(|conn| spp_core::audit::clear_events(conn))" in body,
    )
    check(
        "132: the reader-lane call is gone",
        "with_read(spp_core::audit::clear_events)" not in body,
    )
    check(
        "132: a failure surfaces instead of `unwrap_or(0)` fake success",
        ".unwrap_or(0)" not in body
        and 'self.error = Some(format!("could not clear the event log: {e}"))' in body,
    )
    check(
        "132: the page renders the clear error",
        'design::error_text(ui, &format!("error: {err}"))' in diagnostics
        and "error: Option<String>" in diagnostics,
    )


# ----------------------------------------------------------------- item 133
def verify_item133() -> None:
    palette = read_code(SHELL, "palette.rs")
    ui = function_body(palette, "pub fn ui(&mut self, ctx: &egui::Context, db: &DbHandle)")
    check(
        "133: a full-screen backdrop closes the palette on an outside click",
        'egui::Id::unique("command-palette-backdrop")' in ui
        and "backdrop.clicked()" in ui,
    )
    check(
        "133: a same-frame Enter after a query change confirms nothing",
        "&& !query_changed" in ui
        and "&& let Some(hit) = hits.get(self.selected)" in ui,
    )
    check(
        "133: the suppressed / dry Enter keeps the field focused",
        "if take && (query_changed || take_hits(&self.hits).is_empty())" in ui,
    )
    check(
        "133: the two regression tests ship",
        "fn audit133_same_frame_enter_confirms_nothing_stale" in palette
        and "fn audit133_clicking_outside_the_palette_closes_it" in palette,
    )


# ----------------------------------------------------------------- item 134
def verify_item134() -> None:
    chart = read_code(SHELL, "chart.rs")
    check(
        "134: the concave-unsafe convex_polygon fill is gone",
        "convex_polygon" not in chart,
    )
    check(
        "134: the area fill is an explicit per-segment triangle mesh",
        "pub fn area_fill_mesh" in chart
        and "mesh.indices\n            .extend_from_slice(&[idx, idx + 1, idx + 2, idx + 1, idx + 3, idx + 2]);"
        in chart,
    )
    check(
        "134: the concave-area regression test ships",
        "fn area_fill_mesh_area_matches_the_polygon_even_when_concave" in chart,
    )


# ----------------------------------------------------------------- item 135
def verify_item135() -> None:
    inbox = read_code(PAGES, "inbox.rs")
    mode = function_body(inbox, "pub fn set_composer_mode")
    check(
        "135: switching to note mode spends the provenance",
        "mode == ComposerMode::Note" in mode and "self.inserted_draft = None;" in mode,
    )
    edited = function_body(inbox, "pub fn composer_edited")
    check(
        "135: wiping the composer abandons the draft",
        "body.trim().is_empty()" in edited and "self.inserted_draft = None;" in edited,
    )
    start = inbox.index('"Insert into reply"')
    end = inbox.index("let (gen_ok, has_draft)", start)
    copilot = inbox[start:end]
    check(
        "135: the copilot suggested-reply insert kills any draft claim",
        "self.inserted_draft = None;" in copilot
        and "self.composer_body = report.suggested_reply.clone();" in copilot,
    )
    check(
        "135: the composer edit and mode chips route through the helpers",
        "self.composer_edited(body.clone());" in inbox
        and "self.set_composer_mode(ComposerMode::Note);" in inbox
        and "self.set_composer_mode(ComposerMode::Reply);" in inbox,
    )


# ----------------------------------------------------------------- item 136
def verify_item136() -> None:
    inbox = read_code(PAGES, "inbox.rs")
    check(
        "136: the open popup consumes the arrows BEFORE the TextEdit",
        "let mention_len = caret_query(&self.composer_body, self.mention_caret)"
        in inbox
        and "i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)" in inbox
        and "i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)" in inbox,
    )
    check(
        "136: the arrows move the highlight through move_selection",
        "crate::palette::move_selection(self.mention_active, mention_len, 1)" in inbox
        and "crate::palette::move_selection(self.mention_active, mention_len, -1)" in inbox,
    )
    check(
        "136: the popup only owns the keys while the composer is focused",
        "mention_len > 0 && self.composer_focused" in inbox
        and "self.composer_focused = output.response.has_focus();" in inbox,
    )


def main() -> int:
    for verify in (
        verify_item127,
        verify_item128,
        verify_item129,
        verify_item130,
        verify_item131,
        verify_item132,
        verify_item133,
        verify_item134,
        verify_item135,
        verify_item136,
    ):
        verify()
    print()
    if FAILURES:
        print(f"{len(FAILURES)} check(s) FAILED:")
        for failure in FAILURES:
            print(f"  - {failure}")
        return 1
    print("All batch-32 checks passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
