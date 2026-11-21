# Root-5 — Screenshot vs current UI visual check

Scope: read-only. Inspected all four `/tmp/sk-*.png` files with image reading; compared against
`src/ui.rs` CSS and the widget construction in `src/main.rs`. No source or images modified.

Evidence set (complete; no other `sk-*.png` exist):
- `/tmp/sk-v17.png` — full window, 986×801, mtime 2026-11-21 19:34
- `/tmp/sk-crop-header_controls.png` (947×230), `/tmp/sk-crop-card1.png` (910×165), `/tmp/sk-crop-bottom.png` (947×110) — all mtime 19:41

## Freshness verdict: STALE — the latest screenshot does not show the current UI

`sk-v17.png` predates the working-tree edits by ~2h20m (`src/main.rs` 21:22, `src/ui.rs` 21:51).
Two rendered colors in the screenshot match the **old** CSS and contradict the current source:

| Element | Screenshot | Old CSS (at capture) | Current `src/ui.rs` |
|---|---|---|---|
| `.summary` row | blue `#15539e` on `#353535` | `color: @theme_selected_bg_color` | `color: @theme_fg_color` (line 191) |
| `.session-meta` / `.note` | dim grey, ≈3:1 | `color: @insensitive_fg_color` | `color: alpha(@theme_fg_color, 0.8)` (lines 177, 192) |

Consequence: the two highest-value readability fixes are **in the working tree but visually unverified**.
No screenshot of the current UI exists.

## Ranked visual defects (blocking = prevents readable/usable UI at windowed size)

| # | Sev | Blocking? | Defect | Evidence | Status in working tree |
|---|-----|-----------|--------|----------|------------------------|
| 1 | P0 | **Yes (in capture)** | Summary line effectively unreadable: `#15539e` on `#353535` = **1.61:1** (AA needs 4.5:1). This is "Last check … / nudged / held / watched / need attention" — the primary status line. | pixel sample of `sk-v17.png`; old `ui.rs:189` | **Fixed** in `ui.rs:191`; no fresh screenshot proves it |
| 2 | P1 | No | Secondary text fails AA: `.session-meta` / `.note` ≈3–4:1 at 12–13px (measured peak grey `#919190` ≈4.37:1, most pixels lower). Affects state/idle/model and prompt/nudge lines on all cards. | `sk-v17.png`, `sk-crop-card1.png`; `ui.rs:177,192` | **Fixed** in working tree; unverified |
| 3 | P1 | No | Primary action input has no visible cue: the session-id field renders empty with only a focus ring, no placeholder. "Filter sessions…" next to it does show its placeholder, so the asymmetry is conspicuous. | `sk-crop-header_controls.png`; `src/main.rs:1001` sets `"Session id (ses_...)"` | **Open** — either stale capture or a GTK/CSS placeholder-rendering issue; needs live check |
| 4 | P1 | No | Metadata crowding / overflow risk: each card stacks name+id, a 4-field state line and a 3-field prompt line, with no separators or column alignment. The name label is `set_hexpand(true)` with no ellipsize/max-width, so at narrower windowed widths the Pause button can be pushed off-card. At the captured ~948px width it still fits. | `sk-v17.png`, `sk-crop-card1.png`; `src/main.rs:386-396,412-440` | Open (source-level; not reproducible at captured size) |
| 5 | P1 | No | Redundant destructive control: custom red "Quit" button sits immediately left of the native headerbar close/min/max cluster, with no confirmation despite the note "Closing it stops everything." Visual clutter and misplaced destructive emphasis. | `sk-crop-header_controls.png`; `main.rs:978-983,994` | Open |
| 6 | P2 | No | "Remove" is the only destructive action on each card but is styled identically to Details/Edit prompt/Model/Kick; five same-weight buttons in one row. Destructive affordance is not discoverable. | `sk-crop-card1.png`; action row | Open |
| 7 | P2 | No | Settings and History/log are collapsed bottom rows with tiny disclosure triangles; label lists only four of the contained controls. Low discoverability at windowed height. | `sk-crop-bottom.png`; `main.rs:1082-1288` | Open |

Not blocking for the current working tree: items 2 is fixed; of the remaining open items none
prevents the UI from being read or operated, so **no open defect is blocking**. The only blocking
condition is procedural: the screenshots are stale, so the fixes cannot be signed off visually.

Also confirmed from the capture (not defects): header/title already include the attention count
("Session Keeper - 0 need attention"), cards use status text plus color, badges are legible, and
spacing/padding within cards is adequate at the captured size.

## Skipped / risk
- Did not run the app or capture a new screenshot; overflow at 700×500 and the placeholder-rendering
  question remain unverified. Risk: a "fixed" contrast claim rests on source reading, not pixels.
