# Product Requirements — Session Keeper (Rust)

Owner: Session 3 (product management / requirements, task T003)
Date: 2026-11-21
Model: opencode/hy4-flash-free#high
Status: COMPLETE (analysis performed directly by orchestrator; see blocker note in SESSION_HANDOFFS/session-3.md)

## Source of truth (files actually inspected)

`README.md`, `Cargo.toml`, `src/main.rs` (all 1480 lines), `src/cli.rs`, `src/config.rs`, `src/db.rs`, `src/nudge.rs`, `src/process.rs`, `src/safety.rs`, `src/state.rs`, `src/ui.rs`, all 15 files under `tests/` (68 test functions enumerated). Prior AUTONOMOUS records (`IMPROVEMENT_BACKLOG.md`, `KNOWN_ISSUES.md`, `RESEARCH_LOG.md`, `SECURITY_FINDINGS.md`) were read for context. No speculation: every claim below carries a file:line reference or is explicitly marked as an inferred user need.

---

## 0. Executive summary — key findings

1. **The core watchdog is well built and well tested at the unit level** (classify, decide, debounce, loop detection, caps, gap, away guard, retry, timeout, state persistence are all covered by tests). The product matches its README claims closely.
2. **The single biggest user-need gap is silent failure.** A dead/unmounted drive or DB pauses the entire watchdog with only a log line (`main.rs:80-91`); failed nudges are logged but never alerted (`main.rs:236-242`); the per-session cap is silent (`nudge.rs:82-84`). A non-technical operator can believe the tool is working while nothing is happening.
3. **"Away mode" does not mean "pause all nudges"** — it only suppresses nudges to sessions whose last assistant message ends in `?` (`db.rs:124-126`). This is a semantic mismatch between the label and behavior that needs an explicit product decision.
4. **First-run onboarding has no session discovery.** Adding a session requires knowing a `ses_...` id from an external source; there is no list/browse command or GUI view, and the DB path is hardcoded to one specific drive (`config.rs:3-6`).
5. **CLI and GUI are not at parity** — notably settings can only be changed in the GUI, and import/export paths differ.
6. **Manual "Kick" bypasses every safety gate** (debounce, caps, away guard, min-gap) in both GUI and CLI (`main.rs:514-543`, `main.rs:844-883`).

Priorities below use **P0** (blocks trust/correct use), **P1** (material gap), **P2** (polish/forward-looking).

---

## 1. Current feature inventory (evidence-based)

### 1.1 GUI (`src/main.rs`, `src/ui.rs`)
| Capability | Evidence | Status |
|---|---|---|
| Session cards: status badge, reason tooltip, model, prompt, nudge count, ages | `main.rs:337-572`; `ui.rs:3-21,42-58,89-98` | Implemented |
| Per-session Pause/Resume | `main.rs:398-412` | Implemented |
| Per-session Details dialog (state, model, prompt, ages, recent history) | `main.rs:443-483` | Implemented |
| Per-session Edit prompt | `main.rs:486-496`, `574-615` | Implemented |
| Per-session Model picker (fetch + combo + switch, async) | `main.rs:499-512`, `1333-1391`; `nudge.rs:215-249` | Implemented |
| Per-session Kick with 10 s button cooldown | `main.rs:514-544`, `1407-1436` | Implemented |
| Per-session Remove | `main.rs:546-561` | Implemented |
| Add session with `ses_` prefix, DB existence, duplicate validation | `main.rs:1011-1038` | Implemented |
| Filter by id/title, "Attention only" toggle, priority/name sorting | `main.rs:1005-1009,1061-1079`; `ui.rs:60-87` | Implemented |
| Settings: prompt, min gap, per-session cap, global cap, away mode, tick interval, alert cooldown, global off, dry run, notifications, sort | `main.rs:1082-1178` | Implemented |
| Test notification button | `main.rs:1144-1145` | Implemented |
| Pause all / Resume all | `main.rs:1180-1205` | Implemented |
| Export/Import settings (fixed path `~/session-keeper-export.json`), Reload from disk | `main.rs:1207-1259` | Implemented |
| History + log panel, daily nudge digest, audit export | `main.rs:1262-1288`, `311-322`, `624-634` | Implemented |
| Keyboard: F5 check, Ctrl+F filter, Ctrl+E audit, Ctrl+Q quit | `main.rs:951-973` | Implemented |
| External state edits detected and reloaded every 5 s | `main.rs:1438-1457` | Implemented |
| Stale-watchdog banner + notification | `main.rs:1459-1473` | Implemented (but see gap §5.5) |

### 1.2 CLI (`src/cli.rs`, `src/main.rs:707-913`)
| Mode | Evidence |
|---|---|
| `--check-now` (one tick, persists) | `cli.rs:36`; `main.rs:712-717` |
| `--status [--json]` | `cli.rs:37,72`; `main.rs:718-751` |
| `--watch` (terminal loop, re-reads state each tick) | `main.rs:789-802` |
| `--kick <sid>` | `main.rs:844-883` |
| `--models [filter]` (caps display at 30) | `main.rs:885-897` |
| `--set-model <sid> <id> <provider>` | `main.rs:899-910` |
| `--pause <sid>` / `--resume <sid>` | `main.rs:803-818` |
| `--set-prompt <sid> <text>` | `main.rs:819-833` |
| `--export-state <path>` / `--import-state <path>` | `main.rs:756-788` |
| `--export-audit` | `main.rs:834-843` |
| `--dry-run`, `--version`, `--help`, default GUI | `cli.rs:71-76`; `main.rs:707-711,752-755` |

### 1.3 Watchdog decision logic
- Status taxonomy `Due / Nudged / Waiting / Stalled / Clean` — `db.rs:4-11`; classification `db.rs:101-137`.
- Decision `Nudge / Hold / Skip` — `nudge.rs:6-11,63-91`.
- Debounce (2 idle checks) — `safety.rs:10-12`; `main.rs:181-191`.
- Unchanged-output loop detection via `DefaultHasher` fingerprint — `safety.rs:4-8`; `main.rs:167-179`.
- Per-session and global hourly caps from history — `nudge.rs:28-49`; `main.rs:93,193-203`.
- Min-gap, disabled-session/global-off skip — `nudge.rs:70-84`.
- Question / stopped-session guards — `db.rs:124-135`.
- Dry run — `main.rs:205-209`.
- Retry with jitter on 5xx/429/network/timeout (10 s timeout, 2 attempts) — `nudge.rs:97-153`.

### 1.4 Persistence, config, audit
- Atomic state save (tmp + rename) — `state.rs:129-138`; corrupt-file backup to `.corrupt-<stamp>` — `state.rs:107-127`.
- JSONL nudge history + decision log with rotation (2000 → keep 1000) — `nudge.rs:155-205`.
- Audit export (settings + history + log) to `~/session-keeper-audit.txt` — `main.rs:624-634`; `config.rs:35-40`.
- 11 environment overrides — `config.rs:24-56`; `README.md:38-40`. Hardcoded DB/drive/API/service paths — `config.rs:3-6`.
- Single-instance lock (PID + `/proc/<pid>/cmdline` check, drop-removed) — `process.rs:3-18`; `main.rs:656-705`.

### 1.5 Notifications
- `notify()` via `notify-send`, spawns and reaps helper — `nudge.rs:51-61`.
- Call sites: missing session (1 h/session) `main.rs:106-117`; question `120-134`; stalled `135-149`; global cap `193-203`; nudge sent `231`; stale watchdog `1462-1473`; test button `1144-1145`.
- Configurable alert cooldown (`alert_cooldown_s`, default 300 s) applies to question/stalled only — `state.rs:13,79`; `main.rs:125,140`.

### 1.6 Test coverage (15 integration test files, 68 functions)
- Strong: db classify guards (`db_tests.rs`), decide/caps/gap/away (`nudge_tests.rs`), safety debounce/fingerprint (`safety_tests.rs`), retry/timeout (`retry_tests.rs`, `timeout_tests.rs`), model API (`model_tests.rs`), URL/prompt/filter/sort/rotate/history (`url_tests.rs`, `prompt_tests.rs`), state load/save/corrupt/merge (`state_tests.rs`), CLI parsing (16 tests, `cli_tests.rs`), config env (`config_tests.rs`), process/lock helpers (`process_tests.rs`), UI pure helpers (`ui_tests.rs`, `age_tests.rs`, `title_tests.rs`).
- No automated coverage for: `tick()`/`run_tick()` end-to-end, `notify()`, real GUI event handling, lock contention races, stale-watchdog timer, audit export. (Consistent with `KNOWN_ISSUES.md` and `IMPROVEMENT_BACKLOG.md`.)

---

## 2. User needs / jobs-to-be-done model (inferred; explicitly marked)

Persona: **operator** — runs OpenCode coding sessions, is not a developer, wants long-running agent sessions to keep making progress without constant babysitting, and wants to trust that an automated tool won't wreck them or burn money.

| Need / job (inferred) | Why it matters | Current support |
|---|---|---|
| N1 Keep idle sessions moving | Core purpose | Strong (classify + nudge + caps) |
| N2 Never nudge a session that is waiting on a human question | Avoid corrupt agent state | Partial (trailing-`?` heuristic only) |
| N3 Know what the tool did and why | Trust / auditability | Log + history + receipt panel, but alerts incomplete |
| N4 Be alerted only when human action is needed | Avoid alert fatigue | Partial (question/stalled/missing/cap) |
| N5 Stop everything instantly | Safety / control | Close window, global off, pause all |
| N6 Set it up without developer knowledge | Adoption | Weak (no discovery, hardcoded paths) |
| N7 Recover from outages without losing state | Reliability | Weak (silent pause, silent failures) |
| N8 Use from terminal when no desktop | Flexibility | Good CLI, but settings parity gap |

---

## 3. Operator workflow matrix (non-technical operator)

| Job | Actual steps (evidence) | Prerequisite | Friction / gap | Priority |
|---|---|---|---|---|
| Start watching a session | GUI: obtain `ses_...` id → paste into entry → Add (`main.rs:1011-1038`). CLI: no "add" command; must hand-edit `state.json` or import. | Must know session id from OpenCode | No session browse/list anywhere; CLI cannot add a session | **P0** |
| See what happened | GUI card badge/reason/`last_action` (`main.rs:375-436`); Details (`443-483`); history/log panel (`1262-1288`). CLI `--status` (`718-751`). | Understand badge vocabulary | Reasons are terse codes; no plain-language "why no nudge" for holds | P1 |
| Pause/resume one | GUI per-card button (`398-412`) or CLI `--pause/--resume` (`803-818`) | Exact session id for CLI | CLI requires full id; GUI shows short id only | P2 |
| Pause/resume all | GUI buttons (`1180-1205`) | — | No CLI equivalent (only per-session) | P2 |
| Change prompt/model | GUI Edit/Model (`486-512`); CLI `--set-prompt` / `--set-model` (`819-833`, `899-910`) | Model id+provider for CLI | Model picker needs API reachable; empty result just shows label (`1361`) | P1 |
| Tune safety settings | GUI Settings expander (`1082-1178`) | — | No CLI settings flags; only GUI or hand-editing `state.json` | **P0** |
| Export/import config | GUI fixed-path buttons (`1207-1258`); CLI explicit path (`756-788`) | — | GUI import silently fails on bad file (`1241`); CLI gives precise error (`778`) | P1 |
| Stop everything | Close window (`946`), Ctrl+Q (`958-961`), global off (`1134-1136`), pause all | — | Closing GUI stops watchdog (intended, README:3); documented | — |

**Top workflow gaps (P0):** session discovery, CLI settings parity, surfacing held/silent decisions in plain language.

---

## 4. Safety controls — implemented vs missing needs

| Control | Implemented | Evidence | Weakness / missing user need | Priority |
|---|---|---|---|---|
| Two-check debounce | Yes | `safety.rs:10-12`; `main.rs:181-191` | Streak resets to 0 whenever a tick is not Due (`main.rs:154-159`) — a flapping session may never accumulate | P2 |
| Unchanged-output loop detection | Yes | `safety.rs:4-8`; `main.rs:167-179` | `DefaultHasher` is non-cryptographic/unseeded; compares only the single last assistant text, so A→B→A loops evade it | P1 |
| Per-session hourly cap | Yes | `nudge.rs:28-38,82-84` | **Silent** — no alert when reached; counted from history file which can be rotated/deleted | **P0** |
| Global hourly cap | Yes | `nudge.rs:40-49`; `main.rs:193-203` | Alert uses fixed 1 h window, not `alert_cooldown_s` | P1 |
| Min gap | Yes | `nudge.rs:76-81` | Manual Kick ignores it | **P0** |
| Away mode (question guard) | Yes | `db.rs:124-126` | Only trailing `?` detected; does not pause all nudges while operator is away — label is misleading | **P0** |
| Question / stopped guards | Yes | `db.rs:124-135` | Question detection = `text.ends_with('?')` only; misses bullets, code fences, fullwidth `？` | P1 |
| Dry run | Yes | `main.rs:205-209` | Alerts (question/stalled/missing/cap) still fire in dry run; not documented | P2 |
| Global off switch | Yes | `nudge.rs:70`; `main.rs:1134-1136` | — | — |
| Single-instance lock | Yes | `process.rs:3-18`; `main.rs:680-705` | PID + cmdline race window remains (`SECURITY_FINDINGS.md`) | P2 |
| Read-only DB | Yes | `main.rs:45-59` | — | — |
| Manual Kick safety | **No** | `main.rs:514-544` (GUI), `844-883` (CLI) | Bypasses debounce, caps, min-gap, away guard, question guard; no confirmation; counted in history so it consumes cap | **P0** |
| Undo / rollback of a nudge | No | — | No way to retract an accidental kick | P2 |
| Cost / token budget awareness | No | — | Nudges consume model tokens; no budget guard | P2 |

---

## 5. Failure & recovery expectations

| Failure | Current behavior | Evidence | Reasonable user expectation | Gap | Priority |
|---|---|---|---|---|---|
| Drive unmounted / DB path missing | `open_db()` → None; tick logs "Watchdog paused…" and returns with **fresh** `last_tick_ms`; views empty (cards show "working") | `main.rs:45-59,80-91,276-290` | Visible alert + stale/paused banner, cards show unknown | Silent pause; stale detector defeated | **P0** |
| DB locked/unreadable | Same path as above; `SELECT 1` probe rejects | `main.rs:50-58` | Alert + retry with backoff | Silent | **P0** |
| API dead / 5xx / timeout | 2 attempts w/ jitter, then `Err`; state `last_action="failed …"`, log only; no desktop alert; no nudge counter increment | `nudge.rs:97-153`; `main.rs:236-242` | Alert on repeated failure + retry later | Silent to user | **P0** |
| `notify-send` missing | `Command::spawn` fails silently | `nudge.rs:51-61` | Log/fallback so alerts aren't lost | Silent | P1 |
| `state.json` corrupt | Backup to `.corrupt-<stamp>`, reset to defaults | `state.rs:107-127` | Notify user that settings were reset + backup path | Silent | P1 |
| State save failure | `let _ = save()` ignored at most call sites | e.g. `main.rs:409,557,809,1172` | Surface write failure | Silent | P1 |
| Lock held by alive PID | Refuse to start with message | `main.rs:694-700` | Reasonable, but CLI "already running" is easy to misread as a bug | Docs | P2 |
| History/log rotation race | Read-modify-write of whole file, no locking | `nudge.rs:170-175,199-204`; `IMPROVEMENT_BACKLOG.md:5` | Serialised append | Known issue | P1 |
| GUI blocked on SQLite open | Blocking open on UI thread for Add/Details/kick views | `main.rs:1021,1419`; `IMPROVEMENT_BACKLOG.md:4` | Non-blocking worker | Known perf issue | P1 |
| Worker thread panic | Channel sender dropped; `TryRecvError::Disconnected` ignored; `busy` never reset | `main.rs:294-309,1303-1331` | Reset busy + show error | Watchdog wedged silently | P1 |

---

## 6. Edge-case matrix (explicitly requested)

| Edge case | Exact code path / outcome | Evidence | Required expectation | Priority |
|---|---|---|---|---|
| **Wrong session id (GUI add)** | Rejects if not `ses_*`, not in DB, or duplicate; message in summary label | `main.rs:1019-1038` | Good, but a valid id without `ses_` prefix is wrongly rejected | P2 |
| **Wrong session id (CLI)** | `--kick` prints "not in watch list" and exits 0; `--pause/resume/set-prompt` exit 1; `--set-model` never checks watch list and calls API anyway | `main.rs:844-851,803-833,899-910` | Consistent validation + nonzero exit + "not found in OpenCode" check | P1 |
| **Dead API** | Retry then `Err`; logged, state shows `failed …`; no alert; model list returns empty ("No models returned") | `nudge.rs:97-153,224`; `main.rs:236-242,1361` | Alert after threshold; distinguish "no models" from "API unreachable" | **P0** |
| **Stale session (DB has no idle event / no assistant)** | classify → Clean or Stalled; Stalled alerts on transition; missing session alerts hourly | `db.rs:116-136`; `main.rs:106-149` | Reasonable. But DB outage sets fresh last_tick, masking staleness | **P0** (masking) |
| **User away** | `away_mode` only holds sessions whose last assistant text ends `?`; all other idle sessions still nudged | `db.rs:124-126` | Product decision needed: either suppress all nudges when away, or rename to "question-aware mode" and add a true away switch | **P0** |
| **Cap reached** | Per-session: silent Hold. Global: Hold + hourly desktop alert | `nudge.rs:82-84`; `main.rs:193-203` | Both should alert once with cooldown; UI should show remaining budget | **P0** (per-session) |

---

## 7. Cold-start onboarding

Current first-run experience, from code:
1. No state file → `State::default()` (empty watch list, 30 s tick, 10 s gap, 30/60 caps, notify on) — `state.rs:82-111`.
2. GUI opens showing "No watched sessions. Add a session id below." — `main.rs:566-570`.
3. Operator must supply a `ses_...` id obtained externally; no discovery UI or CLI — `main.rs:1001,1022`.
4. DB/API paths are hardcoded to one machine (`config.rs:3-6`); env overrides exist but are undocumented beyond a bare list (`README.md:38-40`).
5. GTK3 must be present and a display available; `gtk::init().expect("GTK3 failed to initialize")` panics headlessly — `main.rs:915`. CLI modes return before this and are safe.
6. `notify-send` assumed installed — `nudge.rs:53`.

**Onboarding requirements (P0):** (a) a session listing/discovery command and GUI picker; (b) a first-run quickstart that explains where to get a session id, what "idle/question/stalled" mean, and that closing the window stops everything; (c) a startup preflight that reports DB/API/notify-send availability and which env overrides are active; (d) graceful message instead of GTK panic when no display.

---

## 8. CLI vs GUI parity matrix

| Capability | GUI | CLI | Parity note |
|---|---|---|---|
| Add session | Yes (validated) | **No** | P0 — CLI cannot add |
| Watch (continuous) | Yes (window) | Yes `--watch` | Both; watch blocked by lock when GUI open (`KNOWN_ISSUES.md`) |
| Status/reason | Yes (cards) | Yes `--status`, `--json` | CLI richer (machine-readable, cap/gap/away fields `main.rs:725-737`) |
| Kick | Yes (10 s cooldown) | Yes | Both bypass safety gates |
| Model switch | Yes (picker) | Yes (`--set-model`) | CLI needs manual id+provider |
| Pause/resume | Yes (one + all) | Yes (one only) | No CLI pause-all |
| Set prompt | Yes | Yes | — |
| Settings (caps, gap, interval, away, cooldown, off, dry run, notify) | Yes | **No** | P0 — CLI settings gap |
| Export/import state | Fixed path `~/session-keeper-export.json` | Explicit `--export-state/--import-state <path>` | P1 — GUI path not configurable; GUI import silently fails (`main.rs:1241`) |
| Export audit | Yes | Yes | — |
| Models listing | Via picker | `--models [filter]` (display cap 30) | — |
| Test notification | Yes | No | P2 |
| History/log/digest panel | Yes | No | P2 |

---

## 9. Notification expectations

| Event | Current trigger / cooldown | Evidence | Gap |
|---|---|---|---|
| Nudge sent | Every successful nudge | `main.rs:231` | Fine; suppressed in dry run because nudge is suppressed |
| Question asked | Transition into Waiting + `alert_cooldown_s` | `main.rs:120-134` | No re-alert if still waiting after restart (last_status persisted) — probably intended |
| Stalled after nudge | Transition into Stalled + cooldown | `main.rs:135-149` | — |
| Missing from OpenCode | Every 1 h, no status-change gate | `main.rs:106-117` | Fixed cooldown, not configurable |
| Global cap reached | Every 1 h | `main.rs:193-203` | Fixed cooldown, not configurable |
| Per-session cap reached | **None** | `nudge.rs:82-84` | P0 |
| Nudge failed | **None** | `main.rs:236-242` | P0 |
| Watchdog paused (DB/drive down) | **None** (log only) | `main.rs:82-85` | P0 |
| Watchdog stale (no tick >120 s) | Every 5 min | `main.rs:1462-1473` | Masked by DB-outage path (fresh timestamp) |
| State corrupt / save failed | **None** | `state.rs:117-126` | P1 |
| Transport missing (`notify-send`) | **None** | `nudge.rs:51-61` | P1 |

Also: `SESSION_KEEPER_NOTIFY` defaults to true (`config.rs:52-56`) and is overwritten from persisted settings at startup (`main.rs:676-679`), so an env override at launch is always replaced by the stored preference — a surprising behavior worth documenting/P2.

---

## 10. Feature-completeness matrix (README claim → source → tests)

| README claim | Source | Tests | Verdict |
|---|---|---|---|
| Read-only DB, tick 5–300 s (default 30) | `main.rs:45-59,645`; `state.rs:11,93` | `config_tests`, `state_tests` | Verified |
| Debounce, loop detection, caps, min-gap, question/stopped guards | `db.rs:101-137`; `nudge.rs:63-91`; `safety.rs`; `main.rs:167-209` | `db_tests`, `nudge_tests`, `safety_tests` | Verified at unit level |
| Alerts for question/stalled/missing/cap | `main.rs:106-203` | none | Implemented but untested; per-session cap not alerted — Partial |
| notify-send + configurable cooldown | `nudge.rs:51-61`; `state.rs:79` | none | Partial (cooldown applies only to 2 event types) |
| Reason log + nudge history + audit export | `nudge.rs:155-205`; `main.rs:624-634` | `prompt_tests::rotate_keeps_recent_lines` | Verified (export path untested) |
| GUI cards/buttons/settings/shortcuts | `main.rs:337-1476` | `ui_tests` (pure helpers only) | Implemented; interactions not click-verified (`KNOWN_ISSUES.md`) |
| "time-in-state" (README:15) | `main.rs:421-427` shows last-progress + idle ages, not time-in-status | — | Partial / naming |
| CLI command list (README:22-36) | `cli.rs:2-126`; `main.rs:707-913` | 16 `cli_tests` | Verified |
| Env override list (README:40) | `config.rs:24-56` | `config_tests` | Verified |
| "shared with old Python keeper's data" (README:44) | Same `~/.config/session-keeper/` paths | — | Plausible; no explicit migration code — claim is about path reuse only |
| Build `cargo test` / `--release` | `Cargo.toml` | `TEST_RESULTS.md` | Verified per task board (18 binaries, commit b9daf12) |

---

## 11. Prioritized requirements backlog

IDs are stable references for other sessions to pick up. Owner is a *suggestion*; root assigns.

| ID | Requirement | Priority | Suggested owner | Evidence anchor |
|---|---|---|---|---|
| R-001 | Alert + UI banner when DB/drive unavailable; do **not** refresh `last_tick_ms` on a paused tick; show cards as "unknown" | P0 | S6 + S5 | `main.rs:80-91,276-290,1462-1473` |
| R-002 | Desktop alert on repeated nudge failure and on per-session cap reached; add cooldown | P0 | S6 | `main.rs:236-242`; `nudge.rs:82-84` |
| R-003 | Session discovery: CLI `--list-sessions` and GUI picker; document how to obtain a `ses_` id | P0 | S6 + S5 | `main.rs:1001-1038`; `db.rs` |
| R-004 | DECIDED by root (global pause now one click in toolbar): keep "away mode" as question-aware holding only; global off switch and per-session pause are the real pause controls. No semantic change. | either suppress all nudges when operator is away, or rename to "question-aware" and add a true away switch; broaden question detection | P0 | S3 decision + S6 | `db.rs:124-132` |
| R-005 | Make manual Kick an explicit override: confirmation in GUI, and document that it bypasses all gates (or route it through gates) | P0 | S8 + S6 | `main.rs:514-544,844-883` |
| R-006 | CLI settings parity (flags for caps/gap/interval/away/cooldown/off) or document state.json editing | P0 | S6 | `main.rs:1082-1178` |
| R-007 | First-run preflight + graceful no-display error; document GTK3 and notify-send prerequisites | P1 | S5/S10 | `main.rs:915`; `nudge.rs:53`; `config.rs:3-6` |
| R-008 | Check `notify-send` availability and log/fallback when alerts cannot be delivered | P1 | S6 | `nudge.rs:51-61` |
| R-009 | Notify on state-corrupt reset and state-save failure | P1 | S7 | `state.rs:107-138` |
| R-010 | Harden loop detection: seeded hash over last N assistant outputs, not one unseeded hash | P1 | S8/S6 | `safety.rs:4-8`; `main.rs:167-179` |
| R-011 | Persist nudge counters in state (or protect history) so caps survive history rotation/deletion | P1 | S7 | `nudge.rs:28-49` |
| R-012 | DONE: catch_unwind in launch_tick surfaces worker panic in summary and resets busy; disconnect path also resets busy | P1 | S6 | `main.rs:294-309,1326-1327` |
| R-013 | Clarify plain-language hold reasons in UI; expose remaining cap budget | P1 | S5/S4 | `main.rs:68-76,160-163` |
| R-014 | GUI import validation parity with CLI + backup before overwrite + configurable export path | P1 | S5 | `main.rs:1207-1242` |
| R-015 | Configurable cooldowns for missing/cap alerts (use `alert_cooldown_s`) | P1 | S6 | `main.rs:109,196` |
| R-016 | Add integration tests for `tick`/`run_tick`, notify, lock, stale watchdog, audit export | P1 | S9 | test inventory §1.6 |
| R-017 | Consistent CLI session-id validation and exit codes; `--set-model` should check watch list | P1 | S6 | `main.rs:803-910` |
| R-018 | Undo / retract last manual kick | P2 | S6 | — |
| R-019 | Machine-readable status fields: cap remaining, cooldown remaining, next-eligible time | P2 | S6 | `main.rs:725-737` |
| R-020 | Accessibility review of badge colors / status text for color-blind operators | P2 | S4 | `ui.rs:154-167` |

---

## 12. Notes for other sessions

- Session 4 (UI/UX): §3, §8, R-013, R-019, R-020 are directly actionable; no source edits for S4.
- Session 5 (frontend): R-001, R-003, R-007, R-013, R-014 touch `src/ui.rs`/GUI portions of `main.rs` — coordinate with root because GUI code lives in `main.rs` (owned by S1).
- Session 6 (backend/CLI): R-001..R-006, R-008, R-012, R-015, R-017; note `src/nudge.rs`, `src/cli.rs`.
- Session 7 (data): R-009, R-011.
- Session 8 (security): R-005, R-010; lock race already noted in `SECURITY_FINDINGS.md`.
- Session 9 (QA): R-016.
- All decisions about product semantics (R-004) should be confirmed by root before implementation.
