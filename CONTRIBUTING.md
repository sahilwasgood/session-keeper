# Contributing

Thanks for helping improve Session Keeper.

## Development setup

Requires Linux, a Rust toolchain, and GTK3 development headers:

```
sudo apt-get install libgtk-3-dev libglib2.0-dev pkg-config
cargo test
cargo build --release
```

The GUI is GTK3 and runs on the main thread. Long-running work (SQLite reads, API calls, discovery) belongs in a worker thread that reports back through a channel. Never block the GTK main thread with network or database calls.

## Before opening a pull request

- Add or update a test for any behavior change. Integration tests live in `tests/`.
- Run `cargo test --all-targets` and `cargo build --release`.
- Keep the watchdog's safety gates intact: debounce, loop detection, per-session and global caps, min gap, and question/human-input guards. Manual Kick is an explicit override.
- Do not add a background service, tray process, or boot integration. Closing the window stops the watchdog by design.

## Reporting issues

Include the Session Keeper version (`--version`), your OpenCode and GTK3 versions, and the relevant lines from `~/session-keeper.log`. Do not paste credentials from `service.json`.

See `AUTONOMOUS/` for the original design notes, task boards, and research that shaped the project.
