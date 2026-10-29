#!/usr/bin/env python3
"""Traceability gate (RS-21 / R4 — Batch RS-3).

Validates that the R4 traceability matrix
(`docs/audit/acceptance/traceability-matrix.md`) stays honest against the two
sources it claims to trace:

  1. **DC v1.0** (`docs/audit/design-contract-v1.md`) — every DS-C/DS-A id cited
     in the matrix must exist in the frozen contract. One registered alias is
     accepted: `DS-C-04` → the DS-C-03 sheet's MetricTile half (the DC folded
     MetricTile into DS-C-03; the code comments and the lab registry keep the
     DS-C-04 label). The alias is only accepted when the matrix itself
     declares it in its "Registered id alias" table.
  2. **The code** — every route of the app (`PageKey` in
     `crates/app-shell/src/pages/mod.rs`) must carry a matrix section, and every
     golden-lock file the matrix cites must exist under `crates/app-shell/tests/`.
     Sections may also declare the registered harness exception
     ("SPP_SMOKE (harness exception)") — the RS-20h Shell precedent.

Additionally, every `DC: DS-C-xx` doc-tag in `crates/app-shell/src/design.rs`
must resolve to a DC id (the same alias rule) — code never leads the contract.

Run from the repository root:

    python3 scripts/gate_traceability.py           # exits 1 on violation

Exit codes: 0 = clean, 1 = violations found.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

MATRIX = ROOT / "docs/audit/acceptance/traceability-matrix.md"
DC = ROOT / "docs/audit/design-contract-v1.md"
PAGEKEY = ROOT / "crates/app-shell/src/pages/mod.rs"
DESIGN = ROOT / "crates/app-shell/src/design.rs"
TESTS = ROOT / "crates/app-shell/tests"

# Registered alias (G-7): cited id → DC sheet that owns it. Only honored when
# the matrix declares the alias in its "Registered id alias" section.
ALIAS = {"DS-C-04": "DS-C-03"}
ALIAS_DECLARATION = "## Registered id alias"

ID_RE = re.compile(r"DS-[AC]-\d+")
SECTION_RE = re.compile(r"^## (?!Registered id alias|Coverage reconciliation)(.+)$", re.M)
LOCK_RE = re.compile(r"kittest_[a-z0-9_]+\.rs")
HARNESS_EXCEPTION = "harness exception"


def dc_ids() -> set[str]:
    text = DC.read_text(encoding="utf-8")
    return set(ID_RE.findall(text))


def routes() -> dict[str, str]:
    """PageKey variant name → sidebar label (both accepted as section keys)."""
    text = PAGEKEY.read_text(encoding="utf-8")
    body = text.split("pub enum PageKey", 1)[1].split("}", 1)[0]
    names = re.findall(r"^\s{4}([A-Z][A-Za-z]+),\s*$", body, re.M)
    labels = {}
    label_block = text.split("pub enum PageKey", 1)[1]
    for m in re.finditer(r"PageKey::([A-Za-z]+)\s*=>\s*\"([^\"]+)\"", label_block):
        labels[m.group(1)] = m.group(2)
    return {n: labels.get(n, n) for n in names}


def matrix_sections(text: str) -> list[tuple[str, str]]:
    """(section title, section body) for every `## ` block except the preamble."""
    matches = list(SECTION_RE.finditer(text))
    sections = []
    for i, m in enumerate(matches):
        end = matches[i + 1].start() if i + 1 < len(matches) else len(text)
        sections.append((m.group(1).strip(), text[m.end():end]))
    return sections


def section_key(title: str) -> str | None:
    """Map a matrix section title to a PageKey route name (or None to skip)."""
    name = title.split("—")[0].strip()
    name = re.sub(r"\s*\(.*?\)\s*", "", name).strip()
    name = re.sub(r"^\d+\.\s*", "", name)
    return name


def main() -> int:
    violations: list[str] = []
    allowed = dc_ids()
    if not allowed:
        violations.append(f"no DS-C/DS-A ids found in {DC}")

    alias_declared = ALIAS_DECLARATION in MATRIX.read_text(encoding="utf-8")
    matrix_text = MATRIX.read_text(encoding="utf-8")

    def check_ids(where: str, ids: set[str]) -> None:
        for ds_id in sorted(ids):
            if ds_id in allowed:
                continue
            if ds_id in ALIAS:
                if not alias_declared:
                    violations.append(
                        f"{where}: {ds_id} resolves only through the registered "
                        f"alias (→ {ALIAS[ds_id]}), but the matrix does not "
                        f"declare it under '{ALIAS_DECLARATION}'"
                    )
                continue
            violations.append(f"{where}: {ds_id} does not exist in the DC")

    # 1. Every id cited in the matrix must exist in the DC (alias rule applies).
    for title, body in matrix_sections(matrix_text):
        check_ids(f"matrix section '{title}'", set(ID_RE.findall(body)))

    # 2. Every design.rs DC doc-tag must resolve to the DC.
    design_ids = set(ID_RE.findall(DESIGN.read_text(encoding="utf-8")))
    check_ids("design.rs DC tags", design_ids)

    # 3. Every route must carry a matrix section.
    key_to_section: dict[str, str] = {}
    for title, body in matrix_sections(matrix_text):
        key = section_key(title)
        if key:
            key_to_section[key.lower().replace(" ", "")] = title
    for variant, label in routes().items():
        candidates = {
            variant.lower().replace(" ", ""),
            label.lower().replace(" ", ""),
        }
        # "Shell chrome (DC §6)" covers the Shell route.
        if variant == "Shell":
            candidates.add("shellchrome")
        if not (candidates & set(key_to_section)):
            violations.append(
                f"route {variant} ('{label}') has no traceability-matrix section"
            )

    # 4. Every cited golden-lock file must exist (or the harness exception).
    for title, body in matrix_sections(matrix_text):
        for lock in LOCK_RE.findall(body):
            if not (TESTS / lock).exists():
                violations.append(f"matrix section '{title}': golden lock file "
                                  f"{lock} does not exist under {TESTS}")
        if LOCK_RE.search(body) is None and "SPP_SMOKE" not in body:
            violations.append(
                f"matrix section '{title}': neither a kittest golden lock nor "
                f"the registered SPP_SMOKE harness exception is cited"
            )

    if violations:
        print("traceability gate: FAIL")
        for v in violations:
            print(f"  - {v}")
        return 1

    print(
        f"traceability gate: OK — {len(routes())} routes covered, "
        f"{len(design_ids)} design.rs DC tags valid, "
        f"DC id set {len(allowed)} ids, alias declared: {alias_declared}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
