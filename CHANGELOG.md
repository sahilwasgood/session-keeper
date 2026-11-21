# Changelog

## Unreleased

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
