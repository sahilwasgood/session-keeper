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

AT-SPI GUI verification: Check now added log lines; Details dialog opened and closed; Kick confirmation dialog opened; Save settings did not crash; dry-run Kick sent 0 nudges (119 -> 119). External dry-run toggle via CLI also propagated to running GUI within 5s.
Note: an earlier dry-run test before the env-propagation fix sent 2 real nudges to watched sessions; fixed and retested safely.

API response validation: 200 with malformed/non-object body now returns error instead of marking a nudge successful; regression test added. 28 binaries green.

Human-input detection: permission/form endpoints probed live (both {"data":[]}); helper tests cover non-empty and empty arrays. Integrated into tick; 28 binaries green.

AT-SPI regression pass: filter field reduced visible session cards 9 -> 1 -> 9; sort checkbox persisted via Save settings; Discover dialog opened and cancelled; Help expander present. Sort setting restored to false after test.

Event-driven wakeups: SSE /api/event parsed {"type":...} frames; watcher thread signals tick on session.idle/status/permission.asked/form.created, with stop flag on window close. Verified no runaway ticking (0 log lines in 20s between events) and 30s fallback still ticking (9 lines in 35s). 29 test binaries green.

SSE filter verified; 29 test binaries green with event_url regression test.

SSE observability: connection/disconnection logged once, retry errors throttled to once per minute; live restart logged "event stream connected".

Toolbar pause verified via AT-SPI: Pause all -> enabled=false, label "Resume all"; second click -> enabled=true, label "Pause all".

Toolbar pause label drift fixed: the Settings bulk button shadowed the toolbar toggle variable, so label updates hit the wrong widget. Renamed toolbar toggle to pause_toggle; external CLI pause now flips toolbar to "Resume all" (verified via AT-SPI and screenshot) and back.

Worker error handling (R-012): launch_tick wraps run_tick in catch_unwind; a panic now sends an error outcome, the GUI surfaces it in the summary and resets busy so Check now stays usable. Disconnected channel also resets busy and logs. Normal Check now verified via AT-SPI (summary timestamp advanced); all 29 binaries green.

R-008: notify() now checks notify-send availability once (cached), logs a single warning when the transport is missing, and returns bool. GUI Test notification shows a dedicated persistent label ("Test notification sent" verified via AT-SPI); previously feedback was overwritten by the next tick on the shared summary label. Added cached-availability test; 29 binaries green.

R-009: State::load_from_path sets a process-global corrupt flag; main() logs + eprints + notifies once at startup. Verified with an isolated SESSION_KEEPER_STATE corrupt file: stderr warning, log line, timestamped backup preserved. Added assertion in state_corrupt_tests. 29 binaries green.

Question guard: added db::is_question (trim trailing whitespace, recognize fullwidth ?) and used it in both classify branches; added unit + classify-with-trailing-newline tests. 29 binaries green. Fresh release reinstalled and live GUI smoke-checked (summary + Events connected).

Save-failure surfacing: State::save() now logs the first failure once per process (covers all 20 ignored-Result call sites) without caller churn. New save_failure_tests.rs binary verifies exactly one log line on repeated failures; 30 binaries green; live app shows zero spurious warnings.

CLI exit consistency: --kick on an unknown session now exits 1 (was 0); --set-model validates the watch list before calling the API and exits 1. Added cli_exit_tests.rs (spawns binary with isolated paths) covering both; 31 binaries green.

OSS readiness: 31 test binaries green; GitHub Actions CI passed on first public push. Secrets scan clean. Portable defaults verified live: app runs with ~/.config/session-keeper/env overrides and shows local wall-clock time.

API error surfacing: added fetch_models_result/fetch_sessions_result (ureq::Error::Status mapped to HTTP message); GUI Discover/model picker now show real cause instead of "No sessions returned". CLI --sessions/--models exit 1 with the cause. Verified dead-API CLI (exit 1) and AT-SPI GUI label. 31 binaries green.

Preflight: GUI and --watch modes log DB/credentials/notify transport status with concrete paths at startup; one-shot CLI modes skip it to avoid log spam. Verified live log lines and zero new lines from --status.

R-013 partial: tick outcome carries cap_remaining (computed from global cap and hourly history); GUI status line shows "global cap N left" (verified via AT-SPI: 54 left), omitted when unknown/unlimited. 31 binaries green.

R-010: loop detection now catches A->B->A via Session.prev_hash (serde default, migrates old state) and safety::is_repeating_output, with unit test for A->A/A->B->A/non-loop cases. Live app shows no false loop holds; 31 binaries green.
