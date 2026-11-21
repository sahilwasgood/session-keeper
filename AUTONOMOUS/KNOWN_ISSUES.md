# Known Issues

- Model picker and Details dialog interaction not click-verified due to X input stacking.
- CLI status may read a state snapshot a second behind the GUI.
- `--watch` with the GUI open is blocked by lock (intended).
- Synthetic GUI testing requires a better harness (Xvfb or AT-SPI).
