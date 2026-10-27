#!/usr/bin/env python3
"""Batch 21 verification harness (clean-audit items 26-30).

Fresh, batch-specific mechanical verification — NOT the existing test
suite. Every check re-reads the shipped source and asserts the exact fix
landed, mirroring the batch-16..20 harness protocol.

Items:
  26  custom-objects form serialises every field value as a JSON string,
      so number/boolean fields always fail core validation and object
      creation never succeeds
  27  the type builder offers checkbox/url field types the core rejects
      as unknown — types built with them can never hold an object
  28  stale-thread deletion treats one unpaginated threads page as the
      conversation's full thread set (threads past page 1 mass-deleted)
  29  `tags_changed` emitted with previous=null on every re-ingest, tags
      changed or not
  30  a retried saved-reply create re-POSTs the payload and duplicates
      the reply on the platform
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


def read(base: Path, p: str) -> str:
    return (base / p).read_text(encoding="utf-8")


# ----------------------------------------------------------------- item 26
def verify_item26() -> None:
    page = read(SHELL, "pages/custom_objects.rs")
    create = page[page.index("fn create_object(") :]
    create = create[: create.index("fn delete_object(")]
    check(
        "26: the form coerces values against the type's field definitions",
        "coerce_property_value(field_type, value)" in create
        and 'f.field_type.clone()' in create,
        "create_object must look up each key's declared type and coerce",
    )
    check(
        "26: coercion errors surface as form errors before any write",
        "self.error = Some(e);" in create and "return;" in create,
    )
    check(
        "26: the blanket string serialisation is gone",
        "serde_json::Value::String(value.to_string())" not in create,
    )

    coerce = page[page.index("fn coerce_property_value(") :]
    coerce = coerce[: coerce.index("#[cfg(test)]")]
    check(
        "26: numbers become JSON numbers (integers stay integers)",
        'value.parse::<i64>()' in coerce and "Value::from(i)" in coerce,
    )
    check(
        "26: decimals parse as finite f64 (inf/nan refuse loudly)",
        "value.parse::<f64>()" in coerce and "f.is_finite()" in coerce
        and "is not a valid number" in coerce,
    )
    check(
        "26: booleans accept the common truth words and reject the rest",
        '"true" | "1" | "yes" | "on"' in coerce
        and '"false" | "0" | "no" | "off"' in coerce
        and "is not a boolean" in coerce,
    )
    check(
        "26: the object table renders non-string scalars instead of blank",
        "other => other.to_string()," in page
        and "value.as_str().unwrap_or_default()" not in page,
    )


# ----------------------------------------------------------------- item 27
def verify_item27() -> None:
    page = read(SHELL, "pages/custom_objects.rs")
    check(
        "27: the builder offers exactly the core's six canonical types",
        'Self::Text => "text",' in page
        and 'Self::LongText => "long_text",' in page
        and 'Self::Number => "number",' in page
        and 'Self::Date => "date",' in page
        and 'Self::Select => "select",' in page
        and 'Self::Boolean => "boolean",' in page,
    )
    check(
        "27: the phantom checkbox/url variants are gone from the builder",
        "Checkbox," not in page and "Url," not in page,
        "the FieldType enum must not carry the phantom variants",
    )
    builder = page[: page.index("impl FieldDraft")]
    check(
        "27: the FieldType block offers only canonical as_str values",
        '=> "checkbox",' not in builder and '=> "url",' not in builder,
    )
    coerce = page[page.index("fn coerce_property_value(") :]
    coerce = coerce[: coerce.index("#[cfg(test)]")]
    check(
        "27: legacy checkbox fields coerce through the boolean path",
        '"boolean" | "checkbox" =>' in coerce,
    )

    core = read(CORE, "custom_objects.rs")
    validate = core[core.index("pub fn validate_properties(") :]
    validate = validate[: validate.index("fn valid_iso_date(")]
    check(
        "27: the core accepts legacy checkbox as boolean and url as text",
        '"checkbox" => value.is_boolean(),' in validate
        and '"url" => value.as_str()' in validate,
    )
    check(
        "27: unknown field types are still refused (no catch-all)",
        "Unknown field type" in validate,
    )


# ----------------------------------------------------------------- item 28
def verify_item28() -> None:
    real = read(CORE, "helpscout_real.rs")
    lt = real[real.index("async fn list_threads(") :]
    lt = lt[: lt.index("async fn get_customer(")]
    check(
        "28: the threads read follows the HAL next link in a loop",
        "hal_next_path(&v)" in lt and "loop {" in lt,
    )
    check(
        "28: the loop stops when a page has no next link (or is empty)",
        "Some(next) if n > 0 => path = next," in lt and "_ => break," in lt,
    )
    check(
        "28: the single-request shape is gone",
        lt.count("self.request(&path") == 1 and ".await?;" in lt,
        "the loop must issue exactly one request per page",
    )
    check(
        "28: pages accumulate instead of replacing the previous page",
        "out.extend(" in lt,
    )


# ----------------------------------------------------------------- item 29
def verify_item29() -> None:
    sync = read(CORE, "sync.rs")
    upsert = sync[sync.index("pub fn upsert_conversation(") :]
    upsert = upsert[: upsert.index("pub fn upsert_customer(")]
    check(
        "29: the pre-write tag set is captured before the tag rewrite",
        "conversation_tag_names(conn, conv_local)?" in upsert
        and upsert.index("conversation_tag_names(")
        < upsert.index("write_conversation_tags("),
    )
    check(
        "29: the null-previous emission is gone",
        'serde_json::json!(null)' not in upsert,
    )
    check(
        "29: the diff compares normalized tag sets on both sides",
        'serde_json::json!(normalized_tag_set(&prev_tags))' in upsert
        and 'serde_json::json!(normalized_tag_set(&c.tags))' in upsert,
    )
    check(
        "29: the tag-name reader joins the mirror's own tables",
        "SELECT t.name FROM conversation_tags ct" in sync
        and "JOIN tags t ON t.id = ct.tag_id" in sync,
    )
    check(
        "29: normalization sorts and dedupes (reorders are not events)",
        "set.sort();" in sync and "set.dedup();" in sync,
    )


# ----------------------------------------------------------------- item 30
def verify_item30() -> None:
    workers = read(CORE, "workers.rs")
    job = workers[workers.index("async fn run_saved_reply_write_job(") :]
    job = job[: job.index("async fn run_workflow_job(")]
    create = job[job.index('"create" => {') :]
    create = create[: create.index('"update" => {')]
    check(
        "30: the create path pre-reads the platform's saved replies",
        "self.provider.list_saved_replies(mailbox_remote_id).await?"
        in create,
    )
    check(
        "30: the twin is matched on the natural key (name AND text)",
        "r.name == name" in create
        and "r.text.clone().unwrap_or_default() == text" in create,
    )
    check(
        "30: a landed twin skips the re-POST (idempotent create)",
        "if already_landed {" in create and "skipping the re-POST" in create
        and "self.provider" in create,
    )
    check(
        "30: a fresh create still POSTs",
        "create_saved_reply(mailbox_remote_id, &name, &text)" in create,
    )
    # The re-read stage must still exist for every action.
    check(
        "30: the post-write re-read still runs for all actions",
        "let replies = self.provider.list_saved_replies(mailbox_remote_id).await?;"
        in job,
    )


def main() -> int:
    verify_item26()
    verify_item27()
    verify_item28()
    verify_item29()
    verify_item30()
    print()
    if FAILURES:
        print(f"batch 21 harness: {len(FAILURES)} check(s) failed")
        for f in FAILURES:
            print(f"  - {f}")
        return 1
    print("batch 21 harness: all checks pass")
    return 0


if __name__ == "__main__":
    sys.exit(main())
