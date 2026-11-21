# Architecture Decisions

- GTK3 via gtk-rs 0.18, no GTK4/libadwaita port: too heavy for a small utility.
- Theme-aware GTK3 CSS with freedesktop symbolic icons, no bundled icon packs.
- Synchronous worker thread + std mpsc: adequate for one local API and one SQLite file.
- Ticks never replace whole state in the GUI; only tick-owned fields are merged.
- Single-instance lock is a PID file verified against /proc cmdline; no OS flock dependency added.
- State is JSON with serde defaults; malformed files are backed up before fallback.
- On-demand only: no boot services, no tray process left after window close.
