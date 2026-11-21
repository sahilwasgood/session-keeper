# Test Results

2026-11-21 baseline: `cargo test` green across 18 test binaries.
Live verification: CLI status, JSON status, check-now dry run, kick with mock API, model list, audit export, state export/import, external state sync, lock behaviour, GUI screenshot.
GUI synthetic input limitation: XTEST clicks land on terminal window; model/Details dialogs not click-verified.

## 2026-11-21 — Session 9 (T009) test expansion

`cargo test` green: **28 test binaries, 160 tests passing, 0 failed, 0 ignored** (was 18 binaries at baseline). Full suite re-run after integration, twice after concurrent `src/**` edits by other sessions.

Ten new integration-test files (94 new tests), all under `tests/**`:

| File | Focus | Tests |
|---|---|---|
| `tests/cli_edge_tests.rs` | CLI parser edge cases: help precedence, flag-looking positional values, missing args, mode override, HELP text | 18 |
| `tests/state_merge_tests.rs` | `apply_tick_fields` merge semantics, user-field preservation, duplicate/unknown ids, globals, concurrent saves to distinct paths, `states_differ` | 7 |
| `tests/nudge_guard_tests.rs` | min-gap boundaries, per-session cap, unlimited cap, disabled state/session, non-Due statuses, debounce boundaries, fingerprint loop primitive | 12 |
| `tests/model_api_tests.rs` | Mock-server model client tests: empty/malformed payloads, skipped items, HTTP errors, trailing slash, POST payload + Basic auth headers | 9 |
| `tests/lock_pid_tests.rs` | lock file parsing edge cases, `/proc` PID checks, no lock created by Version/Status modes | 12 |
| `tests/state_corrupt_tests.rs` | corrupt-state backup, serde defaults, empty-id filtering, atomic save/no `.tmp`, unknown fields | 7 |
| `tests/api_failure_tests.rs` | 5xx retry to success, single attempt, non-retryable 4xx, 429 retry, connection refused, hung-server timeout | 6 |
| `tests/log_rotation_tests.rs` | `rotate_text` boundaries incl. keep=0/no-trailing-newline; `format_log_time` wraparound incl. negative | 15 |
| `tests/json_status_tests.rs` | Black-box `--status --json` schema/values, empty and malformed state, compact single-line output | 4 |
| `tests/state_sync_integration.rs` | Black-box export/import round-trip, invalid import leaves state intact, export to missing dir fails | 4 |

### Test-findings / limitations for root

- The "unchanged output" loop guard (`hash_same` check) lives in private `main.rs::tick()` and cannot be unit-tested without a refactor; `fingerprint()` stability is covered as its primitive. No source changes requested.
- `rotate_text(text, max, keep)` panics via slice underflow when `keep > text.lines().count()` on an over-limit text (e.g. `max=3, keep=10`). Production callers use max=2000/keep=1000 and never hit it. Not asserted as a passing test; flagged to S7/root as a latent robustness bug.
- `switch_model` error text is non-deterministic across concurrent source edits (ureq `status code NNN` vs crate `HTTP NNN` fallback); test asserts the status code is present rather than an exact prefix.
- Env-var races avoided: process-level `set_var` is not used anywhere in the new files. All env overrides are per-child via `Command::env`; file-based tests use unique temp paths keyed by PID and clean up after themselves.
- During integration, `src/state.rs` was transiently broken by a concurrent session edit (`Option` vs `Result` in `load_from_path`); it was resolved upstream before the final green run. No `src/**` files were edited by Session 9.
- **Current tree state (post-verification, external break):** S5 changed `src/ui.rs::status_tooltip(status: &Status) -> &'static str` (one arg) while `src/main.rs:380` still calls it with two args. `cargo test` now fails at compile time in the bin target. This is not a test failure and is outside Session 9 ownership. Root: drop `&v.reason` at `src/main.rs:380` (or restore the two-arg signature) and re-run `cargo test` to confirm 28/160 again.

Final integration: 28 test binaries green; new tests for URL encoding, invalid session id rejection, lock file private mode, rotate_text underflow. Live lock check: mode 0600, second launch blocked. Release binary 3.8MB.

Final: 28 binaries green after Kick confirmation and preflight; GUI restarted pid 285589, window present, ticking.

CLI --sessions verified against live OpenCode API; session id/title parsing covered by tests.

Failure alert field integrated: 28 binaries green; GUI restarted with final binary.
