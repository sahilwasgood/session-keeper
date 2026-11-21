# Root subagent 4 — CLI workflow tests (SAFE, isolated)

Binary tested: `/home/mint/session-keeper-rust/target/release/session-keeper-rust`
- ELF 64-bit x86-64 PIE, dynamically linked, not stripped. 6,127,008 bytes, built 2026-11-21 21:22.
- Version reported by the binary: `Session Keeper 0.1.0 (Rust)`.

Date run: 2026-11-21. All 13 requested workflows exercised; all exited 0. No failures.

## Safety / isolation (why this was safe)
Every path and endpoint the binary reads/writes was redirected under `/tmp/sk-cli-test` before any command ran. The real `~/.config/session-keeper/state.json`, history, lock, log, and audit files were never referenced. No real nudges could be sent: dry-run was forced, the API was pointed at a dead local port, service credentials JSON was nonexistent, and the SQLite DB/drive were pointed at a nonexistent temp path.

Environment applied to every invocation (via `env`, `HOME` also overridden):
```
HOME=/tmp/sk-cli-test/home
SESSION_KEEPER_STATE=/tmp/sk-cli-test/state/state.json
SESSION_KEEPER_HISTORY=/tmp/sk-cli-test/state/history.jsonl
SESSION_KEEPER_LOCK=/tmp/sk-cli-test/state/keeper.lock
SESSION_KEEPER_AUDIT=/tmp/sk-cli-test/state/audit.txt
SESSION_KEEPER_LOG=/tmp/sk-cli-test/state/keeper.log
SESSION_KEEPER_DB=/tmp/sk-cli-test/state/does-not-exist.db   (does not exist)
SESSION_KEEPER_DRIVE=/tmp/sk-cli-test
SESSION_KEEPER_SERVICE_JSON=/tmp/sk-cli-test/state/no-service.json  (does not exist)
SESSION_KEEPER_API=http://127.0.0.1:9                         (dead local port)
SESSION_KEEPER_DRY_RUN=1
SESSION_KEEPER_NOTIFY=0
```
Seed state placed at `SESSION_KEEPER_STATE`: one fake session `ses_TESTCLI0001` (enabled=true, prompt=null, nudges=0, persist_dry_run=true). The fake id does not exist in any real OpenCode DB.

Proof no real paths were touched — only files created by the whole run (under `/tmp`):
```
/tmp/sk-cli-test/.err
/tmp/sk-cli-test/results.log
/tmp/sk-cli-test/run.sh
/tmp/sk-cli-test/state/audit.txt
/tmp/sk-cli-test/state/exported.json
/tmp/sk-cli-test/state/keeper.log
/tmp/sk-cli-test/state/state.json
```

## Results — exact commands, outputs, exit codes

Notation: every command below was run with the env block above. `stderr` was empty for all commands unless shown.

### 01 — `--help`  → exit 0
```
$ session-keeper-rust --help
Session Keeper
Usage: session-keeper-rust [options]

Options:
  (none)                       Open the window and run the watchdog while it is open
  --check-now                  Run one watchdog tick in the terminal and exit
  --status                     Print session statuses and exit
  --kick <session-id>          Send the nudge prompt to one session now
  --models [filter]            List available models, optionally filtered
  --set-model <sid> <id> <provider>   Switch one session's model
  --pause <sid>                Pause one session
  --resume <sid>               Resume one session
  --set-prompt <sid> <text>    Set that session's nudge prompt
  --watch                      Print status every tick in the terminal
  --version                    Show version
  --export-state <path>        Copy state.json to a backup path
  --import-state <path>        Restore state.json from a backup
  --export-audit               Write history + log to ~/session-keeper-audit.txt
  --json                       Machine-readable status output
  --dry-run                    Never send real nudges
  --help                       Show this help
```

### 02 — `--version`  → exit 0
```
$ session-keeper-rust --version
Session Keeper 0.1.0 (Rust)
```

### 03 — `--status`  → exit 0
```
$ session-keeper-rust --status
_TESTCLI0001  ses_TESTCLI0001  working (working)  model=-  nudges=0  last nudge never ago  last=added
```

### 04 — `--status --json`  → exit 0
```
$ session-keeper-rust --status --json
[{"away_mode":false,"enabled":true,"id":"ses_TESTCLI0001","last_action":"added","last_nudge_ms":0,"min_gap_s":10,"model":"","nudges":0,"reason":"working","session_cap_per_hour":30,"status":"working"}]
```
Valid single-object JSON array; fields match the seeded state.

### 05 — `--export-state`  → exit 0
```
$ session-keeper-rust --export-state /tmp/sk-cli-test/state/exported.json
State exported to /tmp/sk-cli-test/state/exported.json
```
Exported file is valid, pretty-printed JSON containing the one seeded session.

### 06 — `--import-state`  → exit 0
```
$ session-keeper-rust --import-state /tmp/sk-cli-test/state/exported.json
State imported from /tmp/sk-cli-test/state/exported.json (1 sessions)
```
Round-trip succeeded; state reloaded with 1 session.

### 07 — `--pause`  → exit 0
```
$ session-keeper-rust --pause ses_TESTCLI0001
ses_TESTCLI0001 paused
```
Verified by command 08: `"enabled":false` in `--status --json`.

### 08 — `--status --json` (after pause)  → exit 0
```
$ session-keeper-rust --status --json
[{"away_mode":false,"enabled":false,"id":"ses_TESTCLI0001","last_action":"added","last_nudge_ms":0,"min_gap_s":10,"model":"","nudges":0,"reason":"working","session_cap_per_hour":30,"status":"working"}]
```

### 09 — `--resume`  → exit 0
```
$ session-keeper-rust --resume ses_TESTCLI0001
ses_TESTCLI0001 resumed
```
Verified by command 11: `"enabled":true`.

### 10 — `--set-prompt`  → exit 0
```
$ session-keeper-rust --set-prompt ses_TESTCLI0001 "TEST PROMPT hello"
Prompt set for ses_TESTCLI0001
```
Verified by the exported audit (command 12): session `"prompt": "TEST PROMPT hello"`.

### 11 — `--status --json` (after set-prompt)  → exit 0
```
$ session-keeper-rust --status --json
[{"away_mode":false,"enabled":true,"id":"ses_TESTCLI0001","last_action":"added","last_nudge_ms":0,"min_gap_s":10,"model":"","nudges":0,"reason":"working","session_cap_per_hour":30,"status":"working"}]
```
(Note: `--status --json` does not expose the prompt field; prompt mutation confirmed via the audit dump instead. `last_action` remains `added` because `--set-prompt` only writes `prompt`, not `last_action` — consistent with `src/main.rs`.)

### 12 — `--export-audit`  → exit 0
```
$ session-keeper-rust --export-audit
Audit written to /tmp/sk-cli-test/state/audit.txt
```
Audit file written to the overridden audit path (not the real `~/session-keeper-audit.txt`). Content structure:
```
=== SETTINGS ===
{ ...full state JSON with "prompt": "TEST PROMPT hello"... }
=== NUDGE HISTORY ===

=== KEEPER LOG ===

```
History/log sections empty — expected, since no nudges were ever sent in isolation.

### 13 — `--check-now --dry-run`  → exit 0
```
$ session-keeper-rust --check-now --dry-run
Watchdog paused: drive or database unavailable
```
This is the expected result under the isolation setup, not a defect: `open_db()` in `src/main.rs` returns None when the configured drive/DB path does not exist (here deliberately pointed at a nonexistent `/tmp` path). The binary handled it gracefully — logged the message, exited 0, and sent no nudges. It did not attempt the real OpenCode DB.

## Summary
- 13/13 workflows ran; all exit code 0; stderr empty throughout.
- State-changing commands verified by reading state back: pause → enabled=false, resume → enabled=true, set-prompt → prompt stored (seen in audit export), export/import round-trip preserved state.
- `--status` and `--status --json` both produced correct, well-formed output.
- No real nudge was sent (dry-run forced, dead API port, no DB, no credentials).
- No real state/log/audit/lock/config file was read or written; all artifacts are under `/tmp/sk-cli-test`.

## Notes / limitations (not failures)
- Only the happy path for each required flag was exercised. Negative cases (e.g. `--pause` on a session id not in the watch list, malformed `--import-state` file) were not run; source review shows they exit non-zero with a message (`src/main.rs`).
- `--check-now` was intentionally run against a nonexistent DB to guarantee zero real-world side effects, so it reported "drive or database unavailable" rather than a normal tick summary. A tick against a real DB was deliberately not attempted.
- The `--watch` and GUI (no-arg) modes were not run: `--watch` is an infinite loop and the GUI requires a display; neither is in scope for this safe CLI test set.
