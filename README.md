# Session Keeper (Rust)

A small on-demand watchdog for OpenCode sessions. Nothing runs at boot. Open the app when you need it, and the watchdog runs only while the window is open. Closing the window stops everything.

## What it does

- Reads the same state file as the old Python keeper: `~/.config/session-keeper/state.json`.
- Every 30 seconds, checks the OpenCode database and sends the existing nudge prompt to sessions that have gone idle.
- Keeps the old guards: minimum gap, hourly cap, away mode, and never nudges a session that stopped on a question.
- Shows each watched session with status and last action. Buttons: Check now, Add session by id, Pause/Resume, Quit.

## Install

The release binary is installed at `~/.local/bin/session-keeper-rust` and a desktop launcher named Session Keeper is in `~/.local/share/applications/`.

The old Python files and the three systemd units were removed on 2026-11-21, so
this app is now the only Session Keeper. Its state, history and log files are
shared locations in `~/.config/session-keeper/` and `~/session-keeper.log`, so
watched sessions survive.

## Build

```
cargo build --release
cargo test
```

Binary size is about 5.3 MB. GUI uses system GTK3, so the extra runtime cost is small.
