# Improvement Backlog

1. Verify GUI interaction with real X events (synthetic clicks currently land on the terminal).
2. Reduce blocking SQLite opens on GUI actions (review finding M1).
3. Serialise history rotation across writer threads (review finding M2).
4. Consider per-session vs global cap alert granularity (review finding M5).
5. Tray icon remains deliberately excluded: conflicts with close-means-stop requirement.
6. Explore automated GUI smoke test with Xvfb or AT-SPI.
7. Continue external research on watchdog UX and Rust GUI alternatives.
