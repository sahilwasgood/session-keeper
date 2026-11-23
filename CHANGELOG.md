# Changelog

## Unreleased

- `--set-setting timezone|date_style` validates input and rejects invalid values instead of silently rendering UTC.

## v0.4.2 (2026-11-24)

- README screenshot refreshed from the redesigned UI using sanitized demo data.
- Nudge HTTP failures now report `HTTP <code>` consistently with other API errors.

## v0.4.1 (2026-11-24)

- `--doctor` now reports the detected OpenCode storage layout (legacy content array vs v2.4 `session_part` rows).
- Package version aligned with GitHub releases, so `--version` now reports 0.4.1.

## v0.4.0 (2026-11-24)

- Redesigned UI: tinted badges with shape glyphs, urgency accent on card edges, quiet secondary actions with one emphasized Kick button, hairline depth, calmer type scale, focus rings, thin scrollbars, tooltips and preserved list scroll position.
- OpenCode 2.4 compatibility: assistant text is read from `session_part` rows when the legacy `content` array is empty, so question and loop guards keep working. Default paths now cover both `opencode` and `opencode2` install names.

## v0.3.1 (2026-11-22)

- Removed internal development notes from the public repository.
- Status filter and Save settings now redraw the session list immediately.
- Toolbar Pause all and the Settings pause checkbox stay synchronized.
- CI gates rustfmt and Clippy for production code.

## v0.4.0 (2026-11-24)

- Toolbar Pause all and the Settings "Watching enabled" checkbox now stay in sync in both directions and across external CLI edits, so saving settings can no longer silently undo a pause.

## v0.4.0 (2026-11-24)

- Save settings now redraws the session list immediately, so prompt and sort changes appear without waiting for the next tick.

## v0.4.0 (2026-11-24)

- Status filter changes now redraw the session list immediately instead of waiting for the next watchdog tick (up to 30 seconds).

## v0.4.0 (2026-11-24)

- Codebase formatted with rustfmt; production code is Clippy-clean. CI now gates both formatting and production lint.

## v0.3.0 (2026-11-22)

- Configurable timestamp display: device-local by default (or fixed UTC offset), 12/24-hour clock, and selectable date granularity. Same-day entries show time only; older ones include the date.
- CLI parity for timestamp preferences (`--set-setting timezone|clock_24h|date_style`).
- GUI status filter drop-down (working, due, nudged, waiting, stalled, paused, unknown, needs attention).
- Scrolling Settings, History/log, and Help panes; collapsing sections no longer hides the others. F11 toggles full screen; the redundant in-app Quit button is gone.
- `--status --json` now includes global cap, cooldown, and next-eligible fields.

## v0.2.2 (2026-11-22)

- Global hourly cap now backed by a bounded rolling timestamp list in state, so deleting or rotating the history file cannot silently reset the cap.

## v0.2.1 (2026-11-22)

- `--status` and `--status --json` now report `unknown` / `database_unavailable` during a database outage, matching the GUI. Added regression coverage.

## v0.2.0 (2026-11-22)

- Local wall-clock timestamps in logs and status (previously UTC).
- Portable OpenCode defaults plus an optional `~/.config/session-keeper/env` override file.
- Startup preflight logging for database, credentials, and notification transport.
- Clear API failure messages in Discover, model picker, `--sessions`, and `--models` instead of an empty list.
- Remaining global cap in the status line and per-session cap on each card.
- State save failures logged once; corrupt state warning; duplicate backup prevention.
- Worker panic and channel-disconnect recovery; consistent CLI exit codes for unknown sessions.
- Broader question detection (trailing whitespace and fullwidth mark), missing notify-send detection, and a neutral Quit button.
- Loop detection now catches A->B->A output oscillation in addition to unchanged output.
- State import now saves a timestamped backup of the previous state before overwriting.
- Event-stream health indicator now clears on a clean stream close, not only on connection failure.
- Daily nudge digest rolls over at local midnight instead of 00:00 UTC.
- A database outage no longer refreshes the watchdog heartbeat, so the stale-watchdog alert fires as intended.
- New `--doctor` command reports database, credentials, notifier, and API reachability.
- `--check-now --json` emits a machine-readable tick result with summary, cap remaining, and per-session views.
- Missing-session and cap-alert cooldowns are now configurable in Settings and via `--set-setting`, defaulting to one hour.
- Cards show an `unknown` badge with a `database_unavailable` reason during a database outage instead of falsely claiming `working`.

## v0.1.0 (2026-11-22)

First public release.

### Watchdog
- Two-check debounce, unchanged-output loop detection, per-session and global hourly caps, min gap, and question/human-input guards.
- Wakeups from OpenCode SSE events (`session.idle`, `session.status`, `permission.asked`, `form.created`) with a 30-second polling fallback and a visible stream-health indicator.
- Alerts for questions, stalled sessions, missing sessions, cap reached, failed nudges, worker errors, corrupt state, and missing notification transport.

### Interfaces
- GTK3 window with session cards, status badges, Details/Edit/Model/Kick/Remove/Pause actions, filter, attention-only toggle, sorting, one-click Pause all, settings, history/log panel, and daily digest.
- CLI parity: check, status (including JSON), watch, kick, pause/resume, prompt and model changes, session discovery, settings, export/import, audit export, dry run, version, and help.

### Reliability and packaging
- Atomic state saves with corrupt-file backup and warning, log/history rotation, single-instance PID lock, and local-time log timestamps.
- Portable defaults for standard OpenCode installs with optional `~/.config/session-keeper/env` overrides.
- MIT license, CI covering the full test suite and release build, contributor guide, and security policy.
