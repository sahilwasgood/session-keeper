# Known Issues

- Model picker and Details dialog interaction not click-verified due to X input stacking.
- CLI status may read a state snapshot a second behind the GUI.
- `--watch` with the GUI open is blocked by lock (intended).
- XTEST synthetic clicks still land on the wrong window; use AT-SPI harness for GUI tests.

Event-driven detection via /api/event SSE remains unimplemented; 30s polling is retained.
