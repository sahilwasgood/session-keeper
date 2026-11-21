# Root session 7 — Failure simulation evidence

Date: 2026-11-21 (simulation ran ~22:14–22:18 local)
Binary tested: `/home/mint/session-keeper-rust/target/debug/session-keeper-rust`
Binary mtime: `2026-11-21 22:06:43.789569712 +0530` (newest `src/*` mtime was `2026-11-21 22:00:22`, so the binary includes all current source edits at test time).

## Isolation / safety

Everything was temporary and localhost-only. No real state, log, DB, lock, or credential file was read or written; no real nudge was sent.

| Variable | Value used |
|---|---|
| `SESSION_KEEPER_STATE` | `/tmp/opencode/sk-failures/state/{base,checknow,corrupt}.json` |
| `SESSION_KEEPER_DB` | `/tmp/opencode/sk-failures/db/test.db` (hand-built mock) or `/tmp/opencode/sk-failures/db/NO_SUCH.db` |
| `SESSION_KEEPER_DRIVE` | `/tmp/opencode/sk-failures` or `/tmp/opencode/sk-failures/NO_SUCH_DRIVE` |
| `SESSION_KEEPER_API` | `http://127.0.0.1:49401` (500→200), `:49402` (hang), `:49403` (malformed), `:49404` (ok/control), `:49411` (500→200, second counter) |
| `SESSION_KEEPER_LOG` | `/tmp/opencode/sk-failures/log/{keeper,big}.log` |
| `SESSION_KEEPER_HISTORY` | `/tmp/opencode/sk-failures/log/history.jsonl` |
| `SESSION_KEEPER_LOCK` | `/tmp/opencode/sk-failures/lock/keeper.lock` |
| `SESSION_KEEPER_AUDIT` | `/tmp/opencode/sk-failures/log/audit.txt` |
| `SESSION_KEEPER_SERVICE_JSON` | `/tmp/opencode/sk-failures/cfg/service.json` with `{"password":"MOCK_PASSWORD_ONLY"}` |
| `SESSION_KEEPER_NOTIFY` | `0` |

Mock server: `/tmp/opencode/sk-failures/mock_server.py` (Python `ThreadingHTTPServer` bound to `127.0.0.1`, logs every request with timestamp, method, path, Authorization header and body). Harness: `/tmp/opencode/sk-failures/run_tests.sh`. Raw transcript: `/tmp/opencode/sk-failures/report.txt`.

Mock DB schema created with `sqlite3`: `session_message(session_id, type, time_created, data)` and `session_v2(id, title, model)`, with one assistant row at `time_created=1000` and a later `idle` row at `2000` for `ses_FAILTEST`. State session had `idle_streak=2` so the debounce gate passed.

### Baseline (proves the mock DB drives a real "due" session)

`--status` → exit 0, 1.07s:
```
ses_FAILTEST  ses_FAILTEST  due (idle)  model=mock-model  nudges=0  last nudge never ago  last=
```
`--status --json` → exit 0, 0.05s:
```
[{"away_mode":false,"enabled":true,"id":"ses_FAILTEST","last_action":"","last_nudge_ms":0,"min_gap_s":0,"model":"mock-model","nudges":0,"reason":"idle","session_cap_per_hour":30,"status":"due"}]
```

Authorization/body actually sent to the mock (decoded `Basic b3BlbmNvZGU6TU9DS19QQVNTV09SRF9PTkxZ` = `opencode:MOCK_PASSWORD_ONLY`):
```
POST /api/session/ses_FAILTEST/prompt auth=Basic b3BlbmNvZGU6TU9DS19QQVNTV09SRF9PTkxZ body={"text":"FAILED SIM PROMPT"}
```

---

## 1. API returns 500 first, then 200 — retry works

**`--kick ses_FAILTEST`** against `:49401` → exit 0, 0.40s:
```
Kicked ses_FAILTEST
```
Mock log (2 requests; timestamps 0.13s apart):
```
1795279622.222043 POST /api/session/ses_FAILTEST/prompt ... body={"text":"FAILED SIM PROMPT"}
1795279622.354944 POST /api/session/ses_FAILTEST/prompt ... body={"text":"FAILED SIM PROMPT"}
REQUEST COUNT: 2
```
Behavior: first POST got 500, client retried automatically, second got 200, command reported success. The intermediate 500 was **not** printed to stdout/stderr.

**`--check-now`** against a fresh `:49411` counter → exit 0, 0.24s:
```
Last check 16:47:03   nudged 1   held 0
```
Mock log: 2 identical POSTs (timestamps 1795279623.129837 and 1795279623.316421). Same silent retry inside the watchdog tick.

## 2. API hangs — bounded timeout, no indefinite hang

`--kick ses_FAILTEST` against `:49402` (mock sleeps 90s before responding), wrapped in `timeout 35s` → exit 1 (client, not the wrapper), **20.46s**:
```
Kick failed: http://127.0.0.1:49402/api/session/ses_FAILTEST/prompt: Network Error: Network Error: Error encountered in the status line: timed out reading response
```
Mock log: 2 requests, 1795279623.542233 and 1795279633.877473 (~10.3s apart), i.e. two 10s ureq timeouts plus retry. No panic. No infinite hang: the CLI terminated on its own at ~20s. Note the error message duplicates the phrase `Network Error:`; cosmetic only.

## 3. API returns HTTP 200 with malformed JSON body — accepted as success

`--kick ses_FAILTEST` against `:49403` (body `<<<NOT JSON>>>{broken`) → exit 0, 3.48s:
```
Kicked ses_FAILTEST
```
Mock log: exactly 1 request (no retry). **Finding:** `send_nudge_with_timeout` only inspects `resp.status()`; the response body is never parsed, so a 200 with garbage JSON is treated as a successful nudge and the session is marked as kicked in state/history.

## 4. DB path missing (drive present)

`SESSION_KEEPER_DB=/tmp/opencode/sk-failures/db/NO_SUCH.db`.

`--check-now` → exit 0, 0.06s, exact stdout:
```
Watchdog paused: drive or database unavailable
```
The same string was appended to the temp log (`16:47:30 Watchdog paused: drive or database unavailable`). No nudge attempted.

`--status` → exit 0, 0.06s:
```
ses_FAILTEST  ses_FAILTEST  working (working)  model=-  nudges=2  last nudge 2s ago  last=kicked 16:47:28
```
`--status --json` → exit 0:
```
[{"away_mode":false,"enabled":true,"id":"ses_FAILTEST","last_action":"kicked 16:47:28","last_nudge_ms":1795279648624,"min_gap_s":0,"model":"","nudges":2,"reason":"working","session_cap_per_hour":30,"status":"working"}]
```
**Finding:** `--status` reports a healthy-looking `working (working)` with `model=-` when the DB is unavailable. The only explicit "paused/unavailable" signal is on the `--check-now` path (and in the log); status silently degrades.

## 5. Drive path missing (valid DB present)

`SESSION_KEEPER_DRIVE=/tmp/opencode/sk-failures/NO_SUCH_DRIVE`, DB valid.

`--check-now` → exit 0, 0.06s:
```
Watchdog paused: drive or database unavailable
```
`--status` → exit 0, 0.06s:
```
ses_FAILTEST  ses_FAILTEST  working (working)  model=-  nudges=2  last nudge 2s ago  last=kicked 16:47:28
```
Log tail gained a second identical `Watchdog paused` line at `16:47:31`. Behavior is identical to case 4: `open_db()` gates on both drive and DB existence, and the drive check wins before the valid DB is opened. No panic.

## 6. Corrupt state file

`SESSION_KEEPER_STATE=.../state/corrupt.json`, content `{ this is : not json [[[` (25 bytes).

`--status` → exit 0, 0.52s, output was **empty** (no session lines, no diagnostic).
`--check-now` → exit 0, 0.23s:
```
Last check 16:47:32   nudged 0   held 0
```
Backups created next to the corrupt file (original left in place, not overwritten):
```
corrupt.json.corrupt-1795279651857439347
corrupt.json.corrupt-1795279652082905330
corrupt.json.corrupt-1795279652205755303
corrupt.json.corrupt-1795279652205991867
```
**Findings:** corrupt JSON silently falls back to `State::default()` with no stderr warning; there is no user-visible "state was corrupt, backed up to …" message. Two backups are created per CLI invocation (one from the boot `State::load()` in `main`, one from the mode-specific load), so 4 backups after 2 commands. No panic.

## 7. Log file with >2000 lines — rotation works

`.../log/big.log` seeded with 2500 lines (`preexisting log line 0001` … `2500`), then one `--check-now` with DB missing (triggers one `log_line`). Exit 0, 0.53s:
```
Watchdog paused: drive or database unavailable
```
Counts/content:
```
BIG LOG lines before: 2500
BIG LOG first before: preexisting log line 0001
BIG LOG lines after: 1000
BIG LOG first after: preexisting log line 1502
BIG LOG last after: preexisting log line 2500
16:47:32 Watchdog paused: drive or database unavailable
```
Behavior: append then `rotate_text(text, 2000, 1000)` → keeps the newest 1000 lines (oldest 1501 dropped, new line retained). Same helper rotates `history.jsonl` at the same thresholds. No panic, no blocking.

## 8. `--kick --dry-run` — sends nothing

`--kick ses_FAILTEST --dry-run` against the always-200 control mock `:49404` → exit 0, 0.04s:
```
Would kick ses_FAILTEST with: FAILED SIM PROMPT
```
The control mock's request-log file was never even created (`REQUEST COUNT: 0`, shell reported `No such file or directory` for `ok-control.log`) — proof that zero HTTP requests were made and no nudge was sent.

Clarification on the history file: it contained 3 `{"kind":"nudge"}` lines at the end of the run. Those came from the earlier **real-to-mock** kicks in cases 1, 1b and 3 (success paths call `append_history`), not from the dry run. They are localhost mock deliveries, not real nudges.

---

## Summary of observations

- **Panics: none.** No `panicked at`, no segfault, no nonzero exit other than the expected kick failure (case 2, exit 1).
- **Hangs: none indefinite.** The hanging API produced a bounded ~20.46s failure (2 × 10s ureq timeout + retry). Worth noting as a latency budget for any caller that waits on `--kick`.
- Retry on `500` works end-to-end in both `--kick` and `--check-now`; the first failure is invisible to the operator.
- Retry on timeout works too (2 attempts).
- Malformed JSON with HTTP 200 is silently treated as success — no response validation.
- DB-missing and drive-missing both produce the explicit pause message on `--check-now`, but `--status` presents the same condition as healthy `working (working)` with `model=-`.
- Corrupt state is silently replaced by defaults and backed up (twice per invocation), with no operator-facing warning.
- Log rotation at >2000 lines correctly trims to the newest 1000 lines on the next log write.
- `--kick --dry-run` makes zero network requests.

Scope note: this was a black-box CLI simulation against temp files and localhost mocks. GUI mode (`--watch`, the GTK window) was not exercised; neither were DB lock/corrupt-schema variants, concurrent kicks, or state-merge behavior. Because other sessions are editing `src/*` concurrently, re-run this harness after integration to confirm the same behavior on the final binary.

---

Ponytail note: kept the harness to one small Python mock + one bash runner; did not add a persistent test suite or cover the GUI/watch paths. Risk: concurrent source edits may change failure behavior — re-run against the final integrated binary.
