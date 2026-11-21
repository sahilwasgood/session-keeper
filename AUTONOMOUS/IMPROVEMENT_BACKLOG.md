# Improvement Backlog

1. DONE: AT-SPI/pyatspi harness verified Check now, Details, Kick, Save settings, dry-run kick, filter, sort toggle, Discover. XTEST clicks unreliable; use AT-SPI.
2. DONE: Add validation and kick timestamp lookup moved off the GTK main thread; discovery and model fetch already worker-based.
3. DONE: process-wide mutex serialises append+rotation (Session 6).
4. Consider per-session vs global cap alert granularity (review finding M5).
5. Tray icon remains deliberately excluded: conflicts with close-means-stop requirement.
6. DONE: AT-SPI works; Xvfb not installed and not needed.
7. Continue external research on watchdog UX and Rust GUI alternatives.

3. Credential file mode limitation on fuseblk mount: document, not fixable locally.
4. Away-mode semantics decided: keep question-aware label; global off switch is the pause control.

7. DONE: SSE event-driven wakeups with 30s fallback.
