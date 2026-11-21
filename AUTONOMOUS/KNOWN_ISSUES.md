# Known Issues

- Model picker and Details dialog interaction not click-verified due to X input stacking.
- CLI status may read a state snapshot a second behind the GUI.
- `--watch` with the GUI open is blocked by lock (intended).
- XTEST synthetic clicks still land on the wrong window; use AT-SPI harness for GUI tests.

Event-driven SSE wakeups implemented as accelerator; 30s polling retained as fallback.

Resolved: toolbar pause label no longer drifts after external CLI pause/resume. Root cause was Rust variable shadowing between the toolbar toggle and the Settings bulk pause button.

R-012 addressed: worker panic no longer wedges the Check now button; error shown in summary, busy reset. Panic path itself is not unit-tested (requires injecting a panic into run_tick).
