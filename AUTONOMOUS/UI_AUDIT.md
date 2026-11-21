# T004 — UI/UX and Accessibility Audit (Session 4)

Status: COMPLETE (read-only; no source files edited).
Auditor: Session 4 orchestrator. Attempted 10 parallel workers; **0 spawned** — runtime depth limit (see `SESSION_HANDOFFS/session-4-subagents.md`). All ten audits were therefore performed directly.

## Method and evidence
- Read `src/ui.rs` (helpers + full CSS block), `src/main.rs` (entire GUI construction, ~1480 lines), `src/config.rs`, `src/nudge.rs`, `src/cli.rs`.
- Inspected `/tmp/sk-v17.png` (986x801 screenshot, app window ~948x746) and cropped regions `/tmp/sk-crop-{header_controls,card1,bottom}.png`.
- Measured actual rendered colors from the screenshot with Pillow and computed WCAG contrast ratios. No other `sk-*.png` files exist.

## Ranked defects

Severity key: **P0** = blocks readability/usability at target size; **P1** = high user impact; **P2** = medium; **P3** = polish.

| # | Sev | Defect | Evidence / file ref | Minimal fix (for S5) |
|---|-----|--------|---------------------|----------------------|
| 1 | **P0** | Primary status text is nearly invisible: `.summary` uses `@theme_selected_bg_color` as a **foreground**. Screenshot measures text `rgb(21,83,158)` on `rgb(53,53,53)` = **1.61:1** (WCAG AA needs 4.5:1). This is the "Last check / nudged / held / watched / need attention" line. | `src/ui.rs:166`; rendered in `/tmp/sk-v17.png` summary row | Use `@theme_fg_color` (or a purpose-named high-contrast color) for `.summary`; keep `font-weight:bold`. |
| 2 | **P0** | Window cannot reach the 700x500 target: `set_size_request(760,520)` sets a 760x520 **minimum**, larger than 700x500 in both axes. | `src/main.rs:945` (also `set_default_size(960,680)` at :944) | Lower minimum to e.g. `(640,420)`; make the whole content area scrollable (see #7). |
| 3 | **P1** | Secondary text fails AA contrast: `.session-meta` and `.note` use `@insensitive_fg_color`; screenshot samples measure ~**3.0:1** on both card and window backgrounds at only 12–13px. | `src/ui.rs:153,167`; `src/main.rs:415-440,994-997` | Use `@theme_fg_color` with slight opacity or a mid-grey that measurably reaches 4.5:1. |
| 4 | **P1** | Status/error messages are destroyed by the next tick. Every message written to the single `summary` label ("Session id not found…", "Session is already watched", "Kick failed: …", "Model switch failed: …", "Export failed: …", "Checking…") is overwritten within ≤5s by the periodic tick handler, and immediately by the initial launch tick. | writes: `src/main.rs:1023-1035,1048,1216-1241,1399-1431,1453`; clobber: `src/main.rs:1317-1318` poll handler, `1478` initial tick | Add a dedicated persistent status bar / InfoBar not touched by the tick summary; keep `summary` for the periodic line only. |
| 5 | **P1** | Empty-state copy points the wrong way: "Add a session id **below**." The session-id entry is above the list. | `src/main.rs:567` vs controls at `:999-1003`, list at `:1054-1059` | Change "below" → "above". |
| 6 | **P1** | Filter/attention results of zero show a totally blank list with no explanation, because the empty label only triggers on `sessions.is_empty()`, not on an empty filtered set. | `src/main.rs:353-369,566-570` | Add a "No sessions match the filter" placeholder when `items` is empty but sessions exist. |
| 7 | **P1** | Only the session list is inside a `ScrolledWindow`; the Settings and History/log expanders sit outside it in the outer Box. Expanded, they can exceed a short window and be pushed off-screen with no way to scroll to them. | `src/main.rs:985-1059` (scroll wraps `list` only), `:1082-1288` | Put all non-header content (summary/controls/list/settings/diagnostics) in one scrolled container, or give expanders their own scroll. |
| 8 | **P1** | Redundant destructive control: a custom red "Quit" button sits immediately before the standard headerbar close button; quitting has no confirmation even though the on-screen note warns "Closing it stops everything." Adwaita/GNOME HIG reserves destructive styling for rare, confirmed actions and puts window controls at the extreme right. | `src/main.rs:978-983` Quit; `:994` warning note; screenshot header crop | Remove the custom Quit button (standard close already quits via `connect_destroy`), or require a confirmation dialog. |
| 9 | **P1** | Notification gaps: nudge **failures** are only logged, never notified, so the operator is not alerted when the watchdog cannot act; notification subjects are inconsistent (short id / title / full id), and all use the generic title "Session Keeper". | `src/main.rs:236-242` (failure log only); notify calls `:110,128,143,197,231,1468`; `src/nudge.rs:51-61` | Notify on persistent send failure; standardize subject (prefer title, fall back to full id) and consider a more specific notification title. |
| 10 | **P2** | Dialogs are created with `None::<&gtk::Window>` parent, so they are not transient for the main window: they can open behind it, lose focus, and appear as separate Alt+Tab entries. | `src/main.rs:470-475,583-588,1342-1347` | Pass the main `&window` as transient parent (or `set_transient_for`). |
| 11 | **P2** | Keyboard dead ends: Enter in the session-id entry does nothing (add is click-only); the Edit-prompt dialog entry does not activate the Save button; only undocumented global keys exist (F5/Ctrl+Q/Ctrl+F/Ctrl+E), and F5 is non-standard for refresh (GTK convention: Ctrl+R). | `src/main.rs:1000-1003,1011-1039,583-614,951-973` | `connect_activate` on entries; `set_activates_default`/`set_default`; add Ctrl+R; expose shortcuts in UI. |
| 12 | **P2** | Settings are undiscoverable: buried in a collapsed expander at the bottom, its label lists only 4 of ~12 controls, Save gives no confirmation, and there is no "unsaved changes" cue. Pause-all / import / export / audit are also hidden there. | `src/main.rs:1082-1259` (expander, label, save handler `:1154-1177`) | Move to a standard preferences dialog or headerbar menu; show a brief "Settings saved" acknowledgement. |
| 13 | **P2** | CSS hardcodes colors that break light and high-contrast themes and leak across widgets: `.scrolledwindow { background-color:#14181d; }`, `button:hover #364452`, `button:active #46607a`, plus literal badge/button greens and reds. In a light theme the dark scrolled-area background clashes; manual widget overrides bypass Adwaita. | `src/ui.rs:126-127,128-165,168` | Prefer `@theme_*`/Adwaita symbolic colors; remove the `.scrolledwindow` override and theme-variant-dependent literals. |
| 14 | **P2** | Accessibility naming/semantics: "missing" status reuses the red `.badge-stalled` class, pairing text "missing" with stalled color; icon semantics are misleading (Details=`help-about-symbolic`, Model=`preferences-system-symbolic`, Kick=`mail-send-symbolic`); no explicit ATK names/roles are set anywhere. | `src/main.rs:380-383,443,499,514,61-66`; `src/ui.rs:13-21` | Add `.badge-missing`; choose task-appropriate icons; set accessible names/tooltips on icon buttons. |
| 15 | **P2** | Session identifier shown is the **last 12 characters** with no prefix or ellipsis (`B8fvjPRTh3sx`), full id only in a tooltip; session-name label uses `hexpand` without ellipsis/wrap, so a long title can push Pause off the card. | `src/ui.rs:23-28`; `src/main.rs:386-396,378` | Use prefix + truncated tail (e.g. `ses_…XXXX`) and `set_ellipsize`/`set_max_width_chars` on the name label. |
| 16 | **P3** | Manual "Check now" / Kick / Model fetch show no spinner and do not disable their buttons; the `busy` flag silently drops repeated requests with no feedback. | `src/main.rs:294-309,1041-1050,514-543,499-511` | Add a spinner/progress state and set buttons insensitive while in flight. |
| 17 | **P3** | Headerbar title is static "Session Keeper" while the window title is dynamically updated with the attention count; the two disagree. | `src/main.rs:976` vs `:1319` | Update the headerbar subtitle with the same count. |
| 18 | **P3** | History/log expander renders raw JSONL as a wrapping label with no scroll or monospace; 25 lines can be very tall and hard to read. | `src/main.rs:1262-1286` | Use a `GtkTextView` in a `ScrolledWindow` with a monospace font. |
| 19 | **P3** | Sizing inconsistency: CSS `.badge {min-width:84px}` vs widget `set_size_request(118,-1)` wastes horizontal space; card action row packs five labeled icon buttons that crowd at narrow widths. | `src/ui.rs:154-160`; `src/main.rs:378,442-562` | Pick one sizing source; use icon-only buttons with tooltips at narrow widths or a secondary actions menu. |

## Concurrent-edit note (line references)
`src/ui.rs` was already being modified by another session (S5) while this audit was written. Re-checked against the working tree during handoff: the two P0/P1 color defects are **still present** — `.summary` uses `@theme_selected_bg_color` (now line 189) and `.session-meta`/`.note` still use `@insensitive_fg_color` (now lines 176/190). The `.scrolledwindow` hardcoded background (defect #13) remains at line 191; hover/active colors (#13) appear to have been fixed by S5 via `shade(@theme_bg_color, …)`. Line numbers in the table above refer to the pre-edit version read at audit time; re-grep before applying fixes.

## Contrast measurements (from `/tmp/sk-v17.png`)
- `.summary` text `#15539e` on `#353535` → **1.61:1** (fail; defect #1).
- `.note` / `.session-meta` sampled grey ≈ `#6a857f` on `#353535`/`#2d2d2d` → **≈3.0:1** (fail at 12–13px; defect #3).
- Header text `#eeeeec` on `#353535` → 10.56:1 (good).
- Badge pairs all pass AA (4.86–6.93:1), but see defect #14 for semantic issues.

## What is already good
- Headerbar with title/subtitle and standard close button; card grouping with status text (not color-only) and reason tooltips; icon+text action buttons; keyboard refresh/quit/filter/export shortcuts exist; empty list handled at all; settings persisted and import/export available.

## Recommended minimal change set for Session 5 (src/ui.rs only, S5 ownership)
1. Fix `.summary` foreground and insensitive-text colors (#1, #3) — two CSS lines, highest value.
2. Remove `.scrolledwindow` hardcoded background (#13).
3. Correct empty-state copy "below" → "above" is in `src/main.rs` (root/S5 coordination needed — main.rs is owned by S1; coordinate via root).
4. For structural items (#2, #4, #7, #8, #10, #11) root should schedule changes in `src/main.rs`; S5 cannot edit it.

## Items needing live verification ('to verify')
- Exact overflow behavior of expanded Settings/diagnostics at 700x500 (no live GTK session at that size was available).
- Placeholder text "Session id (ses_...)" appears invisible in the screenshot despite `set_placeholder_text` at `src/main.rs:1001` — likely CSS interaction; confirm on a live GTK3 session.
- Real-world effect of GTK theme variants on the symbolic-color overrides.
