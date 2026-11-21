# Session Keeper Rust Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the boot-time Python Session Keeper services with a small native GTK3 Rust window that starts the watchdog only while the window is open and stops everything when the window closes.

**Architecture:** A single Rust binary owns both UI and watchdog. On launch it loads the existing JSON state, starts a GTK3 window, and runs a 30-second timer callback that reads the OpenCode SQLite database and nudges idle sessions through the local OpenCode HTTP API using the same rules as `keeper.py`. On quit the process exits, so no background loop or daemon remains. The three old systemd units are stopped and disabled; the Python files are left untouched as a fallback.

**Tech Stack:** Rust 1.100, `gtk` 0.18 (system GTK3 3.24.41), `rusqlite` with `bundled` feature (no system sqlite dev headers), `serde`/`serde_json`, `ureq`, `glib`. Binary installed to `~/.local/bin/session-keeper-rust`, launched from a `.desktop` file. No browser, no HTTP server, no cmake or extra system packages.

**Spec:** Approved in chat on 2026-11-21. The user wants the app opened manually only when needed, lightweight, and gone completely on close. Feature upgrades are explicitly deferred.

## Global Constraints

- No boot-time services: `session-keeper.timer`, `session-keeper.service`, `session-keeper-dash.service` must be stopped and disabled by the final task.
- Do not modify `/home/mint/session-keeper/keeper.py`, `dashboard.py` or `index.html`.
- Exact runtime paths (copied from keeper.py): DB `/media/mint/WDC_WD10EZRX_932G/opencode/data/opencode.db`, drive mount `/media/mint/WDC_WD10EZRX_932G`, state `~/.config/session-keeper/state.json`, history `~/.config/session-keeper/history.jsonl`, log `~/session-keeper.log`, API `http://127.0.0.1:49374`, credentials file `/media/mint/WDC_WD10EZRX_932G/opencode/config/service.json`.
- Defaults copied verbatim: `default_prompt = "continue, dont stop"`, `min_gap = 10`, `max_nudges_per_hour = 30`, `away_mode = false`, enabled true.
- Watchdog interval 30 seconds. A tick is skipped when the drive is not mounted or DB is missing/locked.
- Never nudge a session whose last assistant text ends with `?`; never re-nudge when no new assistant output followed the previous nudge.
- No em dashes in any prose or UI text.

## Review Focus

- Missing or corrupt `state.json`: app must start with defaults and not crash.
- DB locked or missing: tick must skip and log, never panic.
- Session waiting on a question: must be left alone even when stale.
- No output after previous nudge: guard must hold and not loop.
- App close: no thread, socket, service or systemd unit left behind.

---

### Task 1: Project skeleton and state model

**Files:**
- Create: `Cargo.toml`, `src/config.rs`, `src/state.rs`, `tests/state_tests.rs`

**Interfaces:**
- Consumes: paths and defaults from Global Constraints.
- Produces: `Config` with `db_path()`, `state_path()`, `api_base()`, `password()`; `State::load() -> State`, `State::save(&self) -> std::io::Result<()>`; `Session` struct with `id`, `enabled`, `prompt`, `handled`, `prompted`, `last_nudge`, `nudges`, `last_action`.

- [ ] **Step 1: Write failing tests for state loading**

```rust
#[test]
fn missing_state_uses_defaults() {
    let s = State::load_from_path(Path::new("/tmp/nonexistent-sk-state.json"));
    assert!(s.enabled);
    assert_eq!(s.default_prompt, "continue, dont stop");
    assert_eq!(s.min_gap, 10);
    assert_eq!(s.max_nudges_per_hour, 30);
    assert!(!s.away_mode);
    assert!(s.sessions.is_empty());
}
```

- [ ] **Step 2: Run `cargo test --test state_tests`; expect failure for missing module**
- [ ] **Step 3: Implement `Config` and `State` in `src/config.rs` and `src/state.rs` with serde defaults and atomic save through a `.tmp` file**
- [ ] **Step 4: Add roundtrip test: save a state with one session, reload it, assert same id, prompt and nudges; run `cargo test --test state_tests` until green**
- [ ] **Step 5: `git init`, `git add`, commit "feat: state model and config paths"**

### Task 2: Session status from the OpenCode database

**Files:**
- Create: `src/db.rs`, `tests/db_tests.rs`

**Interfaces:**
- Consumes: `Config`, `Session`; same SQL shape as `/home/mint/session-keeper/keeper.py:215-245` (`last_event`, `last_assistant`, `session_info`).
- Produces: `enum Status { Due, Nudged, Waiting, Stalled, Clean }` and `fn classify(conn: &Connection, sid: &str, state: &State) -> Status`.

- [ ] **Step 1: Write failing test with a temp SQLite fixture**

```rust
#[test]
fn assistant_question_is_waiting() {
    // fixture: one idle event at t=1000, assistant message ending "?" at t=900
    assert_eq!(classify(&conn, "ses_test", &state), Status::Waiting);
}
```

- [ ] **Step 2: Run `cargo test --test db_tests`; expect failure**
- [ ] **Step 3: Implement `classify` using rusqlite with the same queries and guards as keeper.py (`idle` event, assistant timestamp, question check, handled timestamp, no-new-output check)**
- [ ] **Step 4: Add tests for Due, Nudged (no new output), Stalled (idle with no assistant), Clean (no idle event); run `cargo test --test db_tests` until green**
- [ ] **Step 5: Commit "feat: session status classification from opencode DB"**

### Task 3: Nudge engine with guards and history

**Files:**
- Create: `src/nudge.rs`, `tests/nudge_tests.rs`

**Interfaces:**
- Consumes: `State`, `Session`, `Status`, `Config::password()`.
- Produces: `fn decide(state: &State, session: &Session, status: &Status, now_ms: i64) -> Decision` where `Decision::{Nudge, Hold, Skip}`; `fn send_nudge(api: &str, password: &str, sid: &str, text: &str) -> Result<(), String>` using ureq Basic auth; `fn append_history(sid: &str, kind: &str, detail: &str)`.

- [ ] **Step 1: Write failing guard tests**

```rust
#[test]
fn min_gap_holds_nudge() {
    // session.last_nudge = now - 5s, state.min_gap = 10
    assert!(matches!(decide(&state, &s, &Status::Due, now), Decision::Hold));
}
```

- [ ] **Step 2: Run `cargo test --test nudge_tests`; expect failure**
- [ ] **Step 3: Implement `decide` covering min_gap, max_nudges_per_hour, away_mode question, no-new-output after previous nudge, and `enabled=false`**
- [ ] **Step 4: Add tests for hourly cap and away mode; run `cargo test --test nudge_tests` until green**
- [ ] **Step 5: Implement `send_nudge` against the local API with Basic auth and `append_history`; no live API test needed, verify by compiling and by a manual localhost mock during Task 4**
- [ ] **Step 6: Commit "feat: nudge guards and local API client"**

### Task 4: GTK window and on-demand watchdog timer

**Files:**
- Create: `src/main.rs`, `src/ui.rs`

**Interfaces:**
- Consumes: `State`, `classify`, `decide`, `send_nudge`; GTK3 3.24.41 from the system.
- Produces: a window showing each watched session with status and last action; buttons Check now, Add session by id, Quit; a 30-second `gtk::app::add_timeout` callback that calls one tick and redraws the list. No thread or HTTP server.

- [ ] **Step 1: Write a compile-time smoke test that builds the binary**

Run: `cargo build --release`
Expected: binary at `target/release/session-keeper-rust`.

- [ ] **Step 2: Implement the window and timeout callback**

Timeout callback must set a reentrancy flag, open the DB with `rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY`, classify each enabled session, apply guards, send approved nudges, save state, append history, update the list, and clear the flag. Errors are logged to `~/session-keeper.log` and shown in a small status label.

- [ ] **Step 3: Manual launch check on the desktop**

Run: `target/release/session-keeper-rust`
Expected: window opens immediately; session list populated from `state.json`; clicking Check now logs a tick; closing the window removes the process (`pgrep -f session-keeper-rust` returns nothing).

- [ ] **Step 4: Commit "feat: on-demand GTK window with 30s watchdog"**

### Task 5: Remove boot-time services and install the app

**Files:**
- Create: `~/.local/bin/session-keeper-rust` (copy of release binary), `~/.local/share/applications/session-keeper-rust.desktop`
- Modify: systemd enabled state only (no unit files edited).

- [ ] **Step 1: Stop and disable the old stack**

```bash
sudo systemctl disable --now session-keeper.timer session-keeper.service session-keeper-dash.service
```

Expected: `systemctl is-enabled` returns `disabled` for all three; `pgrep -af session-keeper` shows only the new Rust app when it is running.

- [ ] **Step 2: Install binary and desktop launcher**

Copy the release binary to `~/.local/bin/session-keeper-rust`. Desktop entry: `Name=Session Keeper`, `Exec=/home/mint/.local/bin/session-keeper-rust`, `Type=Application`, `Terminal=false`, no `Autostart` entry.

- [ ] **Step 3: Verify lifecycle twice**

Launch from the desktop menu, wait 35 seconds, confirm one tick in `~/session-keeper.log`; close the window and confirm no process remains and no tick lines are added afterwards. Reboot check: after reboot, `pgrep -af session-keeper` must be empty until the app is opened.

- [ ] **Step 4: Commit install notes and final state**

```
git add . && git commit -m "chore: install on-demand session keeper app"
```

## Self-Review

Spec coverage: on-demand lifecycle in Tasks 4 and 5; reuse of state and guards in Tasks 1 to 3; no browser and no boot service in Global Constraints and Task 5. Step scan: each step has one checkable result. Types: `State`, `Session`, `Status`, `Decision` are consistent across tasks. Review Focus: each risk has a test in Tasks 1 to 3 except app close, which is verified manually in Task 4 Step 3 and Task 5 Step 3. Proportion: plan is shorter than the Rust code it describes.
