# Session 4 handoff — UI/UX & design system (T004)

Role: Session 4 orchestrator (UI/UX and accessibility audit). Charter ownership: `AUTONOMOUS/UI_AUDIT.md`, this handoff, and `session-4-subagents.md` only. **No source code edited.**

## Status
T004 COMPLETE. Deliverable written: `AUTONOMOUS/UI_AUDIT.md` — 19 ranked defects (P0–P3) with file:line evidence, measured contrast ratios, screenshot findings, and minimal fixes.

## Workers
- Model required and used on every spawn attempt: `opencode/hy4-flash-free#high`.
- **Actual worker count: 0.** Ten parallel `subagent` calls were attempted and all failed with `Subagent depth limit reached (1)` — Session 4 is itself a depth-1 subagent, so the runtime forbids nested agents. No reassignment was possible. All ten assigned audits (A–J) were executed directly by the orchestrator; see `session-4-subagents.md` for the full ledger.
- Blocker owner: root/S1 — raise `experimental.subagent_depth` ≥ 2 if future sessions need delegation (config change, not made here).

## Tasks completed
1. CSS/theme contrast (measured actual pixels: summary text **1.61:1**, insensitive text ≈3.0:1 — both fail WCAG AA).
2. Typography at 700x500 (min window size 760x520 makes the target unreachable).
3. Layout overflow (controls/card/settings rows; non-scrolled expanders).
4. Badge/icon/label accessibility (missing-status color mismatch, misleading icons, no ATK names).
5. Focus/keyboard (dialogs non-transient, Enter not wired, undocumented shortcuts).
6. Screenshot inspection (`/tmp/sk-v17.png` + 3 crops; only screenshot present).
7. Empty/loading/error states (wrong-direction empty copy, blank filtered state, errors clobbered by tick).
8. Notification wording/coverage (inconsistent subjects, no failure alerts).
9. Settings discoverability (collapsed bottom expander, no save feedback).
10. Adwaita/freedesktop comparison (redundant Quit vs standard close, hardcoded colors, legacy dialogs).

## Top outcomes for root
- **P0 #1** `src/ui.rs:166` — `.summary` uses `@theme_selected_bg_color` as foreground → primary status text at 1.61:1. Two-line CSS fix; highest value/lowest risk.
- **P0 #2** `src/main.rs:945` — `set_size_request(760,520)` blocks reaching 700x500.
- **P1 #4** `src/main.rs:1317-1318` — periodic summary overwrites all transient error messages.
- **P1 #8** `src/main.rs:978-983` — redundant, unconfirmed destructive Quit beside standard close.
- Ownership note: only `src/ui.rs` is S5's file (#1, #3, #13 can be fixed there). Structural fixes (#2, #4, #7, #8, #10, #11) are in `src/main.rs` (S1-owned) and need root scheduling.

## Blockers / limits
- `src/ui.rs` shows uncommitted changes from another session (S5) during this audit — likely in-progress T005 fixes. Session 4 did **not** touch it. The audit's line numbers reference the pre-edit version; re-checked key CSS lines during handoff (summary/insensitive colors still unfixed). See concurrent-edit note in `UI_AUDIT.md`.
- 0 subagents (runtime depth limit) — audits done directly, documented above.
- Only one screenshot (`/tmp/sk-v17.png`, 986x801); no live GTK at 700x500 to visually confirm expansion overflow and invisible placeholder text — marked "to verify" in the audit.
- No source edits permitted, so all fixes are recommendations only.

## Next tasks
- **Root/S1**: triage `UI_AUDIT.md`; schedule #2, #4, #7, #8, #10, #11 in `src/main.rs`.
- **S5**: apply CSS-only fixes #1, #3, #13, #19 in `src/ui.rs`; keep `cargo test` green.
- **S9**: add regression checks for empty-filter state and error-message persistence if UI tests are added.
- **S2/S3**: cross-reference notification wording (#9) and settings discoverability (#12) with requirements/research logs.
- **S10**: confirm theme behavior under light/high-contrast GTK themes if a runtime is available.
