#!/usr/bin/env python3
"""No-simulation gate (R7 / D14).

Fails the build when production Rust code introduces simulated, fake or
demo data markers. The blueprint's acceptance criterion: "the gate is
green, no user-visible string contains the words demo or simulated".

What is scanned: every .rs file under crates/*/src, excluding
  * comment lines (stripped line starts with //, /// or //!),
  * #[cfg(test)] regions (unit-test fixtures may seed labelled rows),
  * explicit allowlist entries below (each with a justification).

Everything else — string literals, identifiers, SQL text — must be free of
the markers. Run from the repository root:

    python3 scripts/gate_no_simulation.py           # exits 1 on violation

Exit codes: 0 = clean, 1 = violations found.
"""

from __future__ import annotations

import sys
from pathlib import Path

MARKERS = ("simulated", "fake", "demo")

# Each entry: (path relative to repo root, exact stripped line content
# that is allowed to carry the marker, justification).
ALLOWLIST: dict[str, dict[str, str]] = {
    "crates/core/src/audit.rs": {
        '"DELETE FROM application_settings WHERE key IN (\'demo_mode\', \'demo_content_purged\')",':
            "the m050 purge itself: production code whose job is deleting retired demo-mode settings rows",
        '"m050: removed retired demo-mode settings rows"':
            "log message of the same purge — it reports the removal, it does not fabricate",
    },
}


def is_comment(line: str) -> bool:
    stripped = line.lstrip()
    return stripped.startswith("//")


def strip_test_regions(lines: list[str]) -> list[str]:
    """Blank out `#[cfg(test)]` regions, preserving line numbers.

    Every consumed line becomes an empty string (not a removal), so the
    line numbers the scanner reports match the real file. Two subtleties
    the naive scan gets wrong:

    * a `#[cfg(test)]` marker inside a // comment is not an attribute;
      comment lines stay in the output for the comment skip to handle,
    * `#[cfg(test)] mod x;` declares a module without braces — the
      region ends at that declaration line instead of running on into
      the next `{`-bearing production line.
    """
    out: list[str] = []
    i = 0
    n = len(lines)
    while i < n:
        line = lines[i]
        if "#[cfg(test)]" not in line or is_comment(line):
            out.append(line)
            i += 1
            continue
        start = i
        depth = 0
        started = False
        while i < n:
            if not started and is_comment(lines[i]):
                i += 1
                continue
            depth += lines[i].count("{") - lines[i].count("}")
            if "{" in lines[i]:
                started = True
            i += 1
            if started and depth <= 0:
                break
            if not started and ";" in lines[i - 1] and " mod " in f" {lines[i - 1]}":
                break
        out.extend("" for _ in range(start, i))
    return out


def scan(root: Path) -> list[str]:
    violations: list[str] = []
    for path in sorted(root.glob("crates/*/src/**/*.rs")):
        rel = str(path.relative_to(root))
        text = path.read_text(encoding="utf-8", errors="replace")
        lines = strip_test_regions(text.splitlines())
        for lineno, line in enumerate(lines, start=1):
            if is_comment(line):
                continue
            lowered = line.lower()
            if any(m in lowered for m in MARKERS):
                entry = ALLOWLIST.get(rel, {})
                if line.strip() in entry:
                    continue
                matched = [m for m in MARKERS if m in lowered]
                violations.append(
                    f"{rel}:{lineno}: marker {matched!r}: {line.strip()[:120]}"
                )
    return violations


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    violations = scan(root)
    if violations:
        print("no-simulation gate: FAIL", file=sys.stderr)
        for v in violations:
            print(f"  {v}", file=sys.stderr)
        print(
            "\nProduction code must not fabricate data. If the marker is an\n"
            "honest historical reference or removal machinery, add it to the\n"
            "allowlist in scripts/gate_no_simulation.py WITH a justification.",
            file=sys.stderr,
        )
        return 1
    print("no-simulation gate: clean")
    return 0


if __name__ == "__main__":
    sys.exit(main())
