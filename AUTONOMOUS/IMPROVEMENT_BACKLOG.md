# Improvement Backlog

1. Verify GUI interaction with real X events (synthetic clicks currently land on the terminal).
2. DONE: Add validation and kick timestamp lookup moved off the GTK main thread; discovery and model fetch already worker-based.
3. DONE: process-wide mutex serialises append+rotation (Session 6).
4. Consider per-session vs global cap alert granularity (review finding M5).
5. Tray icon remains deliberately excluded: conflicts with close-means-stop requirement.
6. Explore automated GUI smoke test with Xvfb or AT-SPI.
7. Continue external research on watchdog UX and Rust GUI alternatives.

3. Credential file mode limitation on fuseblk mount: document, not fixable locally.
4. Away-mode semantics decided: keep question-aware label; global off switch is the pause control.
