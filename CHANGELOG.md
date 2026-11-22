# Changelog

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
