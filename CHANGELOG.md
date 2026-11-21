# Changelog

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
