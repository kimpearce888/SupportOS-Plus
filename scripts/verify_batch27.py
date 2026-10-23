#!/usr/bin/env python3
"""Batch 27 verification harness (clean-audit items 77-86).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact fix
landed, mirroring the batch-16..26 harness protocol.

Items:
  77  sync invalidation silently reverted unsaved Settings edits
  78  Inbox hard-capped at 50, no pagination
  79  row click silently wiped the typed reply draft
  80  approvals attributed to hardcoded user 5
  81  known-issue links accepted dangling ids and counted them
  82  Customers page fetched page 1 only
  83  LIKE input not escaped despite the ESCAPE clause
  84  jobs::fail called with literal job id 0
  85  rating fetch errors swallowed; ratings permanently lost
  86  panic containment marked copilot jobs retryable
"""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CORE = ROOT / "crates" / "core" / "src"
SHELL = ROOT / "crates" / "app-shell" / "src"
FAILURES: list[str] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    status = "OK " if ok else "FAIL"
    print(f"[{status}] {name}" + (f" — {detail}" if detail and not ok else ""))
    if not ok:
        FAILURES.append(f"{name}: {detail}")


def read(root: Path, p: str) -> str:
    return (root / p).read_text(encoding="utf-8")


def read_code(root: Path, p: str) -> str:
    """Source without doc-comment lines (///) — for checks whose needle
    may legitimately appear inside a comment describing the pre-fix
    defect."""
    return "\n".join(
        line for line in read(root, p).splitlines() if not line.lstrip().startswith("///")
    )


# ----------------------------------------------------------------- item 77
def verify_item77() -> None:
    settings = read(SHELL, "pages/settings.rs")
    check(
        "77: the page tracks the last persisted General snapshot",
        "general_persisted: Option<GeneralSettings>," in settings,
    )
    check(
        "77: invalidation marks the page stale instead of dropping the form",
        "self.stale = true;\n        self.helpscout = None;" in settings,
    )
    check(
        "77: the old drop-the-form invalidation is gone",
        "self.general = None;\n        self.helpscout = None;" not in settings,
    )
    check(
        "77: reload keeps a dirty form via the snapshot guard",
        "is_some_and(|g| self.general_persisted.as_ref() != Some(g))" in settings,
    )
    check(
        "77: a successful save refreshes the snapshot",
        "self.general_persisted = Some(general.clone());" in settings,
    )
    check(
        "77: a pending invalidation refreshes the mirrors on render",
        "if self.general.is_none() || self.stale {" in settings,
    )


# ----------------------------------------------------------------- item 78
def verify_item78() -> None:
    inbox = read(SHELL, "pages/inbox.rs")
    design = read(SHELL, "design.rs")
    check(
        "78: the inbox page size is a named constant",
        "pub const INBOX_PAGE_SIZE: u32 = 50;" in inbox,
    )
    check(
        "78: the offset compiles from the page index",
        "offset: Some(page.saturating_mul(INBOX_PAGE_SIZE))," in inbox,
    )
    check(
        "78: the page carries pager state",
        "page: u32," in inbox,
    )
    check(
        "78: a past-the-end page clamps to the last page with rows",
        "self.page = (total - 1) / INBOX_PAGE_SIZE;" in inbox,
    )
    check(
        "78: the pager renders under the list",
        "let delta = design::pager(ui, self.page, pages);" in inbox,
    )
    check(
        "78: filter changes reset the page",
        inbox.count("self.page = 0;") >= 5,
        f"{inbox.count('self.page = 0;')} resets found (need >= 5)",
    )
    check(
        "78: the subtitle reports the visible range past the cap",
        'format!("{from}-{to} of {} conversations", self.total)' in inbox,
    )
    check(
        "78: the shared pager widget exists in the design system",
        "pub fn pager(ui: &mut egui::Ui, page: u32, pages: u32) -> i32 {" in design,
    )


# ----------------------------------------------------------------- item 79
def verify_item79() -> None:
    inbox = read(SHELL, "pages/inbox.rs")
    check(
        "79: the page carries per-conversation composer drafts",
        "drafts: std::collections::HashMap<i64, StashedDraft>," in inbox,
    )
    check(
        "79: the stash models the composer state",
        "struct StashedDraft {" in inbox,
    )
    check(
        "79: selecting another row stashes the outgoing draft",
        "self.drafts.insert(" in inbox,
    )
    check(
        "79: reselecting restores the stash",
        "if let Some(stash) = self.drafts.get(&id).cloned() {" in inbox,
    )
    check(
        "79: a successful submit spends the stash",
        "self.drafts.remove(&id);" in inbox,
    )
    check(
        "79: the same-row click touches nothing",
        "let switching = self.selected != Some(id);" in inbox,
    )


# ----------------------------------------------------------------- item 80
def verify_item80() -> None:
    automation = read(SHELL, "pages/automation.rs")
    check(
        "80: the decision resolves the real acting user",
        "spp_core::notifications::me_user_local_id" in automation,
    )
    check(
        "80: the approvals carry the resolved actor",
        "spp_core::automation::approve(conn, approval_id, actor)" in automation,
    )
    check(
        "80: the hardcoded user-5 attribution is gone",
        "spp_core::automation::approve(conn, approval_id, 5)" not in automation
        and "spp_core::automation::reject(conn, approval_id, 5)" not in automation,
    )


# ----------------------------------------------------------------- item 81
def verify_item81() -> None:
    radar = read(SHELL, "pages/issue_radar.rs")
    intel = read(CORE, "intelligence_features.rs")
    check(
        "81: a link materializes only for an existing conversation",
        "SELECT ?1, c.id, 'human' FROM conversations c WHERE c.id = ?2" in radar,
    )
    check(
        "81: the create flow validates ids before linking",
        '"SELECT 1 FROM conversations WHERE id = ?1"' in radar,
    )
    check(
        "81: skipped unknown ids are reported to the user",
        "unknown id(s)" in radar,
    )
    check(
        "81: the parsed ids dedupe",
        "if !parsed.contains(&id) {" in radar,
    )
    check(
        "81: the count joins the conversations table (honest total)",
        "conversation_count = (SELECT COUNT(*) FROM known_issue_links kil" in intel
        and "JOIN conversations c ON c.id = kil.conversation_id\n                                  WHERE kil.known_issue_id = ?1)" in intel,
    )
    check(
        "81: the raw VALUES link insert is gone",
        "INSERT OR IGNORE INTO known_issue_links (known_issue_id, conversation_id, link_type)\n             VALUES (?1, ?2, 'human')" not in radar,
    )


# ----------------------------------------------------------------- item 82
def verify_item82() -> None:
    customers = read(SHELL, "pages/customers.rs")
    check(
        "82: the customers page size is a named constant",
        "const CUSTOMERS_PAGE_SIZE: i64 = 50;" in customers,
    )
    check(
        "82: the search fetches the pager's page",
        "                self.page,\n                CUSTOMERS_PAGE_SIZE," in customers,
    )
    check(
        "82: the page carries pager state",
        "page: i64," in customers,
    )
    check(
        "82: a past-the-end page clamps to the last page with rows",
        "self.page = (total + CUSTOMERS_PAGE_SIZE - 1) / CUSTOMERS_PAGE_SIZE;" in customers,
    )
    check(
        "82: the pager renders under the table",
        "let delta = design::pager(" in customers,
    )
    check(
        "82: a new search resets the page",
        "self.page = 1;\n                self.reload_search(db);" in customers,
    )
    check(
        "82: the card title reports the visible range past the cap",
        'format!("{from}-{to} of {} customers", self.total)' in customers,
    )
    check(
        "82: the hard-coded page 1 fetch is gone",
        "                1,\n                50," not in customers,
    )


# ----------------------------------------------------------------- item 83
def verify_item83() -> None:
    search = read(CORE, "search.rs")
    inbox = read(CORE, "inbox.rs")
    customers = read(CORE, "customers.rs")
    mirror = read(CORE, "mirror_readouts.rs")
    check(
        "83: escape_like is a shared public helper",
        "pub fn escape_like(q: &str) -> String {" in search,
    )
    check(
        "83: the inbox query filter escapes the search text",
        'format!("%{}%", crate::search::escape_like(query))' in inbox,
    )
    check(
        "83: the customers search escapes the query",
        'format!("%{}%", crate::search::escape_like(query))' in customers,
    )
    check(
        "83: the saved-replies mirror search escapes and declares ESCAPE",
        "crate::search::escape_like(q)" in mirror
        and "LIKE '%' || lower(?1) || '%' ESCAPE '\\\\'" in mirror,
    )
    check(
        "83: the raw inbox interpolation is gone",
        'let q = format!("%{query}%");' not in inbox,
    )


# ----------------------------------------------------------------- item 84
def verify_item84() -> None:
    workers = read_code(CORE, "workers.rs")
    check(
        "84: draft_article receives the job id",
        "self.run_draft_article_job(job_id, &payload).await?;" in workers,
    )
    check(
        "84: fetch_hs_report receives the job id",
        "self.run_fetch_hs_report_job(job_id, &payload).await?;" in workers,
    )
    check(
        "84: no jobs::fail call carries literal id 0 anymore",
        "jobs::fail(&conn, 0," not in workers,
    )


# ----------------------------------------------------------------- item 85
def verify_item85() -> None:
    workers = read(CORE, "workers.rs")
    ratings_arm = workers[workers.index('"sync_conversation_ratings"') :]
    ratings_arm = ratings_arm[: ratings_arm.index('"embed_knowledge_chunks"')]
    check(
        "85: the rating fetch outcome is matched, not swallowed",
        "match self.provider.get_rating(rating_id).await {" in ratings_arm,
    )
    check(
        "85: a fetch error propagates (the queue retries)",
        "return Err(e);" in ratings_arm,
    )
    check(
        "85: an absent rating is an honest logged no-op",
        "rating absent platform-side - nothing to store" in ratings_arm,
    )
    check(
        "85: the swallowing let-chain is gone",
        "let Ok(Some(r)) = self.provider.get_rating(rating_id).await" not in workers,
    )


# ----------------------------------------------------------------- item 86
def verify_item86() -> None:
    workers = read(CORE, "workers.rs")
    containment = workers[workers.index("job panicked (contained at worker boundary)") :]
    containment = containment[: containment.index("drop(conn);")]
    check(
        "86: the panic containment fails terminally",
        "false," in containment,
    )
    check(
        "86: the retryable containment is gone",
        '"job panicked (contained at worker boundary)",\n                    true,' not in workers,
    )
    check(
        "86: the rationale is recorded at the fix",
        "re-persisted" in workers,
    )


def main() -> int:
    for verify in (
        verify_item77,
        verify_item78,
        verify_item79,
        verify_item80,
        verify_item81,
        verify_item82,
        verify_item83,
        verify_item84,
        verify_item85,
        verify_item86,
    ):
        verify()
    print()
    if FAILURES:
        print(f"{len(FAILURES)} check(s) FAILED:")
        for failure in FAILURES:
            print(f"  - {failure}")
        return 1
    print("All batch-27 checks passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
