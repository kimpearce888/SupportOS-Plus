# progress.md — active items

> Exactly five active items at any time, per `rule.md`. Items move here from `plan.md` (Rule 2)
> and leave for `completed.md` only after verification (Rule 5).

## Active items

**Batch 9 — audit fixes (items 40-44, from the 2026-10-19 read-only audit)**

**40. Enable the `sandbox` feature in the shipped AppImage build (audit P1-1)**
- The Landlock+seccomp parse sandbox is real and tested but opt-in, and
  `xtask package` built without it — the hardening never reached users
  (audit evidence: 0 sandbox strings in the shipped binary). Turn it on in
  the packaging build; verify the artifact actually contains it.

**41. Fix `Cargo.toml` repository/homepage URLs (audit P2-3)**
- Workspace metadata points at `SupportOs-PlusPlus`; the real repo is
  `SupportOs-Plus`.

**42. README: document runtime dependencies honestly (audit P2-4)**
- "no extra system dependencies" is optimistic: the shell needs X11,
  `libxkbcommon-x11`, and a GL/Vulkan implementation (Mesa) at runtime —
  present on standard desktops, absent on minimal/headless systems (proven
  empirically in the audit). Also document that the shipped AppImage
  includes the parse sandbox (item 40).

**43. Remove the dead `Error::NotImplemented` variant (audit P2-5)**
- Defined in `core/src/error.rs`, never constructed anywhere.

**44. Align `decisions.md` "all 24 pages" wording with the real page map (audit P2-6)**
- The port has 21 `PageKey` entries; the original's 24 page files all have
  counterparts (docs folded into knowledge, command_palette → palette
  overlay, not_found not needed natively). Say so precisely.
