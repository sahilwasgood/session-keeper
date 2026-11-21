# Root Integration Review — Sessions 5, 6, 7, 9 (read-only)

Author: Session 1 (root) integration support subagent
Date: 2026-11-21
Model: opencode/hy4-flash-free#high
Scope: read-only review of current git state + source layout; no source edits made.

---

## 1. Current git / diff state

**Baseline at the start of this review:** HEAD `b9daf12`, clean tracked tree, only `?? AUTONOMOUS/`. `TEST_RESULTS.md` records `cargo test` green across 18 test binaries.

**Live update during the review:** S5/S6/S7/S9 landed their work into the working tree *while this review was running* (the harness restarted repeatedly as they wrote). The final observed state is:

```
 M src/cli.rs    M src/db.rs    M src/nudge.rs    M src/state.rs    M src/ui.rs
?? tests/api_failure_tests.rs      ?? tests/cli_edge_tests.rs
?? tests/json_status_tests.rs      ?? tests/lock_pid_tests.rs
?? tests/log_rotation_tests.rs     ?? tests/model_api_tests.rs
?? tests/nudge_guard_tests.rs      ?? tests/state_corrupt_tests.rs
?? tests/state_merge_tests.rs
```

So this document now serves two purposes: (a) the standing contract checklist below, and (b) a concrete verdict on the actually-landed diff — see **Section 8**, which is the authoritative current finding. The tree was still being edited on the final check, so Section 8 is a snapshot; re-run `cargo test` at merge time.

## 2. Ownership map (from PROJECT_CHARTER.md) — the central integration constraint

| Session | Owns | Does NOT own |
|---|---|---|
| S5 frontend | `src/ui.rs` only | `src/main.rs` (GUI wiring lives here) |
| S6 backend/CLI | `src/nudge.rs`, `src/cli.rs` | `src/main.rs` (all consumers of these APIs) |
| S7 data | `src/state.rs`, `src/db.rs` | `src/main.rs` (all producers/consumers of these types) |
| S9 QA | `tests/**` only | any source file |
| S1 root | `src/main.rs`, `Cargo.toml`, commits | — |

**Critical structural risk:** almost every public type/function owned by S5/S6/S7 is *consumed in `src/main.rs`*, which only root may edit. A change that alters a struct field, enum variant, or function signature in an owned file **cannot compile as a standalone merge** — it requires a same-commit root patch to `main.rs`. `PRODUCT_REQUIREMENTS.md:283` already flags this for S5. Plan every merge as an atomic (source + main.rs + affected tests) unit, never as a drop-in file replacement.

---

## 3. Cross-file contract inventory (what must stay consistent)

### 3.1 `State` / `Session` fields — `src/state.rs`

- `State` (state.rs:49-80): 15 fields, with serde defaults at field level; hand-written `Default` impl at 82-102.
- `Session` (state.rs:16-46): 16 fields, `#[derive(Default)]`, per-field `#[serde(default)]`.
- **Every explicit `Session { .. }` literal that must be updated if a field is added** (all list all 16 fields, none use `..Default::default()`):
  - `src/db.rs:102-111` — `classify()` fallback session.
  - `src/main.rs:1029-1032` — GUI "Add session" push.
  - `tests/db_tests.rs:5-16` — `session()` helper.
  - `tests/nudge_tests.rs:5-16` — `sess()` helper.
  - `tests/state_tests.rs:23-35` — roundtrip fixture.
  - `tests/state_tests.rs:83-89` uses `..Default::default()` — the *only* resilient literal.
- **Default-value divergence trap:** `Session` uses derived `Default` (new numeric fields = 0, bool = false), while deserializing an older `state.json` that lacks the field uses the field's `#[serde(default = "...")]`. If S7 adds a field (e.g. a persisted nudge counter for R-011), these two values must be made equal or "new session" and "loaded legacy session" will behave differently. `State` avoids this only because it has a manual `Default` impl — keep that pattern if adding `State` fields.
- `states_differ()` (state.rs:141-143) compares full serde strings; adding fields changes the comparison surface used by the 5-second external-edit reload at `main.rs:1438-1457`. Semantics stay correct only if every consumer serializes the same struct version.
- Tests pinning defaults: `tests/state_tests.rs:4-13` (default_prompt `"continue, dont stop"`, min_gap 10, cap 30, away false). Changing defaults requires S7 + S9 coordination.

### 3.2 `TickFields` / `GlobalTickFields` / `apply_tick_fields` — `src/state.rs`

- Defined at state.rs:145-167; merge logic at 169-189.
- **Producers (all in `main.rs`, root-owned):** `tick()` early-return at 85-89; per-session `updates` map at 248-262; `globals` at 263-267. `TickOutcome` carries them (main.rs:34-39) and `run_tick()` returns them (271-292).
- **Consumer:** `main.rs:1310` applies them on the UI thread after each worker tick.
- **Test:** `tests/state_tests.rs:80-115` constructs a full `TickFields` literal and asserts a user edit (`enabled=false`, custom prompt) survives the merge.
- **Risk:** if S7 adds a per-session field that must survive a tick (R-011 nudge counters, R-009 corrupt/save alert timestamps), it must be threaded through `TickFields` + `apply_tick_fields` + **both** `main.rs` producers + the S9 test literal. Four files, three owners. Silently omitting the merge step means the GUI worker overwrites the new field with a stale snapshot on every tick — a data-loss bug that unit tests on `State` alone will not catch.

### 3.3 `Mode` / `Options` — `src/cli.rs`

- `Mode` (cli.rs:2-18): 16 variants. `Options` (21-26): `mode, dry_run, json, explicit_dry`. `parse_args` (28-115), `HELP` (117-126).
- **Consumers in `main.rs`:** `resume_mode` at 680; `lock_needed` `matches!` at 682-693; exhaustive `match opts.mode` at 707-913.
- **Adding a `Mode` variant is a compile error in `main.rs` until root adds a match arm.** Adding a state-mutating CLI flag (R-006 settings parity) also requires adding the mode to `lock_needed` at 682-693 or two processes will race on `state.json`.
- `explicit_dry` drives the startup env precedence at `main.rs:667-675` (`--dry-run` overrides persisted `persist_dry_run`). Any S6 change to this logic changes real nudge behavior and must be paired with root + S9.
- Tests pinning exact variants: `tests/cli_tests.rs` (16 tests, e.g. lines 10, 17, 36, 48, 55, 70, 74, 85, 91-92, 98, 106, 113, 120-124). `HELP` text is untested but documented in README and must be updated in cli.rs by S6.

### 3.4 `Status` enum — `src/db.rs`

- Defined at db.rs:4-11: `Due, Nudged, Waiting, Stalled, Clean`. `classify()` at 101-137.
- **Fan-out on any variant change — five consumers across four owners:**
  - `src/ui.rs:3-21` (`status_text`, `status_class`), `60-81` (`sort_key`, `needs_attention`, `status_priority`) — S5.
  - `src/nudge.rs:63-91` (`decide` holds unless `Status::Due`) — S6.
  - `src/main.rs:68-76` (`reason()`), `119-152` (alert transitions), `279-286` (`SessionView`), `357-369` (sort/filter), `723/743` (JSON/text status) — root.
  - `tests/db_tests.rs`, `tests/nudge_tests.rs` — S9.
- A new `Status` variant that S5 forgets to map in `ui.rs` will silently render with no badge class; one that S6 forgets in `decide()` will silently change nudge gating. Treat `Status` as a frozen vocabulary unless all five sites move together.

### 3.5 `ModelInfo` and GUI channels — `src/nudge.rs` + `src/main.rs`

- `ModelInfo { id, provider, name }` at nudge.rs:208-213; produced by `fetch_models()` at 215-236 and consumed by:
  - `MODEL_TX`: `OnceLock<Sender<(String, Vec<ModelInfo>)>>` at main.rs:20, set at 935, read with `.expect(...)` at 501, pumped at 1338-1391.
  - GUI dialog at 1348-1362 encodes selection as `"{id}|{provider}"` and matches current model by `m.id == current`.
  - `tests/model_tests.rs`.
- `KICK_TX`: `OnceLock<Sender<(String, Result<(), String>)>>` at main.rs:21, set at 938, read at 526, pumped at 1407-1436. Payload is `(sid, Result<(), String>)`; the result arm updates `handled/prompted/last_nudge/nudges/idle_streak/last_action` at 1416-1428.
- `switch_tx/rx` is a *local* channel (main.rs:936, 1366, 1393-1405) — same `(sid, Result)` payload but no `OnceLock`.
- **Risk:** changing `ModelInfo` fields (S6) breaks the GUI dialog and the `id|provider` key contract in root's `main.rs`. Adding any new async GUI action (e.g. R-003 session-discovery picker) requires a new `OnceLock` + `glib::timeout_add_local` pump, all root-owned. S5/S6 cannot do this alone.

### 3.6 `Decision` enum and nudge public API — `src/nudge.rs`

- `Decision { Nudge, Hold, Skip }` at nudge.rs:6-11; `decide()` at 88-91; `decide_with_history()` at 63-86.
- Consumed in `main.rs:99-165` (the whole `decision != Decision::Nudge` branch) and `tests/nudge_tests.rs`. A new decision variant must be handled in `main.rs` and tested by S9.
- Public functions imported by `main.rs:7`: `append_history, decide, fetch_models, log_line, notify, nudges_in_last_hour_all, send_nudge, switch_model`. Also `send_nudge_with_timeout/retry`, `prompt_url`, `rotate_text`, `nudges_in_last_hour` are used by tests (`retry_tests`, `timeout_tests`, `url_tests`, `prompt_tests`). Signature changes = root + S9 edits.
- **History JSONL schema** `{ "t", "sid", "kind", "detail" }` written at nudge.rs:155-176 is consumed by three modules: `nudge.rs` counters (28-49), `ui.rs::history_for_sid` (30-40), `main.rs::today_digest`/`write_audit` (311-322, 624-634). Renaming keys or `kind` values fans out to three owners + tests.

### 3.7 `ui.rs` pure helpers — `src/ui.rs`

- `status_text/status_class/short_id/history_for_sid/format_age/sort_key/status_changed/needs_attention/status_priority/matches_filter/prompt_display/summary_line` at ui.rs:3-98; `CSS` at 100-169.
- All are called from `main.rs` (notably `refresh()` at 337-572 and the result pump at 1303-1331); `CSS` is loaded at 919.
- Tests pinning them: `tests/ui_tests.rs`, `age_tests.rs`, `title_tests.rs`, `prompt_tests.rs`, `url_tests.rs`.
- **Risk:** changing a helper signature (e.g. adding "remaining cap" for R-013) requires root to update every `main.rs` call site and S9 to update tests. S5 cannot ship such a change standalone.

### 3.8 DB query/schema assumptions — `src/db.rs`

- `session_message` columns: `session_id, type, time_created, rowid, data` (JSON: `content[].text` with `type=="text"`, plus `outcome`); `session_v2` columns `id, model` (JSON with `id`), `title`. See `last_event` 13-24, `last_assistant` 26-55, `session_model` 57-64, `session_title` 81-87.
- `tests/db_tests.rs:18-33` builds an in-memory fixture with the exact `session_message` DDL. If S7 changes queries, S9 must mirror the schema in the fixture or the tests will fail for the wrong reason (missing column vs. logic change).
- R-004 (broaden question detection beyond `text.ends_with('?')`, db.rs:124-132) touches `classify()` in S7's file but changes nudge gating consumed by S6's `decide()` and root's alert transitions — another three-owner change.

---

## 4. Ranked integration risks

1. **R1 — main.rs ownership bottleneck (highest).** Standalone merges from S5/S6/S7 that change any public signature/field will not compile. Gate: every merge must include a root `main.rs` patch in the same commit.
2. **R2 — `Session` field addition breaks five explicit literals** (db.rs, main.rs, three test files). Silent compile failure if any site is missed; semantic failure if derived `Default` and serde default diverge.
3. **R3 — tick merge drops new per-session state.** A new field added to `Session` but not to `TickFields`/`apply_tick_fields`/the two `main.rs` producers is overwritten on every GUI tick. This is data loss, not a compile error — the most dangerous class of integration bug here.
4. **R4 — `Status` vocabulary change fans out to five sites across four owners.** Partial changes cause silent badge/gating regressions.
5. **R5 — CLI `Mode` addition requires a `main.rs` match arm** (compile error) and a `lock_needed` entry (single-instance race if forgotten).
6. **R6 — GUI channel payload / `ModelInfo` shape change** breaks root-owned model dialog and `OnceLock` pumps.
7. **R7 — tests land last but pin the old contract.** If S9 writes tests against the pre-merge contract and source merges afterward, tests become the blocker. Conversely, if source merges first, `cargo test` is red until S9 updates literals — acceptable only if atomic.
8. **R8 — default-value drift.** Changing `default_prompt`, `min_gap`, caps, `tick_interval_s`, or `alert_cooldown_s` requires keeping state.rs defaults, the GUI spinbutton defaults (main.rs:1086-1142, hardcoded `"continue, dont stop"` at 1156), and state_tests.rs assertions in sync.

## 5. Concrete merge order (each step is one atomic commit; `cargo test` must be green at every gate)

**Gate 0 — baseline.** Confirm `cargo test` green on HEAD `b9daf12` (18 binaries). No source diff currently exists; nothing to replay.

**Step 1 — S7 data foundation first (`state.rs`, `db.rs`).**
Rationale: `State`, `Session`, `TickFields`, `Status` are the types every other module compiles against. Merge them before consumers so consumers are written against the final vocabulary.
Required companions in the same commit:
- Root: update all `main.rs` `Session` literals (1029-1032), `TickFields` producers (85-89, 248-262), `apply_tick_fields` call site (1310), and any `Status` arms in `reason()` (68-76) / alert blocks (119-152) / `SessionView` (279-286).
- S9: update `tests/db_tests.rs`, `tests/nudge_tests.rs`, `tests/state_tests.rs` literals + default assertions.
- Verify: new `Session` fields have identical derived-`Default` and serde-default values; every new persisted field is in `TickFields` + `apply_tick_fields`.

**Step 2 — S6 backend/CLI (`nudge.rs`, `cli.rs`).**
Depends on Step 1's `Status` and `State`.
Required companions:
- Root: new `Mode` arms in `main.rs:707-913`; add state-mutating modes to `lock_needed` (682-693); adapt `Decision` handling (99-165), `ModelInfo` dialog (1333-1391), `KICK_TX` payload handling (1407-1436), and any nudge API signature changes at the `main.rs:7` import.
- S6 updates `HELP` (cli.rs:117-126) in the same commit.
- S9: update `cli_tests.rs`, `nudge_tests.rs`, `retry_tests.rs`, `timeout_tests.rs`, `url_tests.rs`, `prompt_tests.rs`, `model_tests.rs` as needed.
- Verify: all `Mode` variants handled; `--dry-run`/`explicit_dry` env precedence at 667-675 preserved; history JSONL keys unchanged (or all three consumers updated).

**Step 3 — S5 frontend (`ui.rs`).**
Depends on Step 1's `Status`.
Required companions:
- Root: update all `ui.rs` helper call sites in `main.rs` (337-572, 1303-1331) and CSS load (919) if signatures or class names change.
- S9: update `ui_tests.rs`, `age_tests.rs`, `title_tests.rs`, `prompt_tests.rs` literals/signatures.
- Verify: every `Status` variant still maps to a `status_text`, `status_class`, and `status_priority` value; `needs_attention` still matches root's attention counts (1315, 1444).

**Step 4 — S9 full test expansion (R-016).**
Land *after* Steps 1-3 freeze the contracts, so new integration tests target the final API. Add coverage for `tick()`/`run_tick()` outcomes, channel disconnect/busy reset, stale-watchdog timer, audit export. Tests must not edit source to make a test pass.

**Step 5 — root final integration.** Re-run `cargo test`, update README/CHANGELOG/TEST_RESULTS/GLOBAL_TASK_BOARD, single final commit. Do not squash away the per-step green gates — they are the bisect points.

**Alternative if a step cannot be kept green atomically:** publish all changed public type definitions in one root-authored "contract bump" commit that includes `main.rs` + every owned file + every affected test, then layer behavior changes on top. Never merge a type change and its consumer fix in separate commits.

## 6. Cross-file consistency checklist (tick before each commit)

- [ ] Every `Session { .. }` literal lists all fields **or** uses `..Default::default()` — check db.rs:102, main.rs:1029, db_tests.rs:6, nudge_tests.rs:6, state_tests.rs:23.
- [ ] New `Session`/`State` field: derived/manual `Default` value == `#[serde(default = "...")]` value; old `state.json` still deserializes.
- [ ] Every new persisted per-session field appears in `TickFields`, `apply_tick_fields`, both `main.rs` producers, and the S9 merge test.
- [ ] `Status` variants covered in `ui.rs` (text/class/priority/needs_attention), `nudge.rs::decide`, `main.rs::reason` + alert blocks + `SessionView`, and DB tests.
- [ ] New `Mode` variant: `parse_args` branch, `HELP` text, exhaustive `main.rs` match arm, `lock_needed` list if it mutates state, and a `cli_tests.rs` assertion.
- [ ] `Options.explicit_dry` / `dry_run` env precedence at main.rs:667-675 unchanged or deliberately re-tested.
- [ ] `Decision` variants handled in `main.rs:99-165` and `nudge_tests.rs`.
- [ ] `ModelInfo` fields and the `"{id}|{provider}"` combo key still align between nudge.rs:208-213 and main.rs:1348-1362; `MODEL_TX`/`KICK_TX` payloads unchanged or their pumps re-tested.
- [ ] History JSONL keys `{t,sid,kind,detail}` consistent across nudge.rs counters, `ui.rs::history_for_sid`, `main.rs::today_digest`/`write_audit`, and prompt/url tests.
- [ ] Default values consistent across state.rs defaults, GUI spinbuttons/entry (main.rs:1086-1142, 1156), and state_tests.rs:4-13.
- [ ] DB fixture DDL in `tests/db_tests.rs:20-24` matches any changed SQL in `db.rs`.
- [ ] `cargo test` green (baseline: 18 binaries) at the end of every step.

## 7. Notes / risks the operator should know

- The single largest risk is **ownership, not logic**: the charter gives S5/S6/S7 no access to `main.rs`, where all their APIs are consumed. Without root-authored companion patches inside each merge, the branch will not compile. This was not re-scoped or softened — it is a concrete integration finding.
- This review did not run `cargo test` (read-only mandate); the green baseline is taken from `TEST_RESULTS.md` and task board T001. Every merge gate above requires the integrating session to actually run it.
- No source, config, or other AUTONOMOUS file was modified; this document is the only file written.

---

## 8. Live diff verdict (authoritative — actual landed changes)

This section records the real cross-session integration state observed after S5/S6/S7/S9 wrote their files. It overrides the "nothing to merge" conclusion that was true only at review start.

### 8.1 Build/test status at final observation

**The working tree does not compile.** `cargo test` fails at the library stage:

```
error[E0308]: mismatched types
  --> src/state.rs:118-120
   match State::from_json(&text) {   // now returns Result<State, _>
       Ok(s) => s,                    // expected Option<State>, found Result<_,_>
       Err(_) => { ... }
   }
```

Root cause: an in-flight S7 edit changed `State::from_json` from `Option<State>` to `Result<State, _>` while `load_from_path` still destructures `Ok/Err`. This is a classic partial-merge collision of the kind predicted in risk R1/R7 — one file's signature changed without its single consumer being updated. **Merge blocker #1; fix inside `state.rs` before anything else.**

An earlier snapshot (before that edit) compiled and ran, and exposed a second, independent failure:

```
tests/cli_edge_tests.rs:87  resume_consumes_a_flag_looking_value ... FAILED
  left: Help   right: Resume("--json")
```

Root cause: S9's test asserts `--resume --json` interprets `--json` as the session-id value, while S6's new `valid_value()` guard in `cli.rs:55-70` rejects any value starting with `-` and returns `Mode::Help`. **Merge blocker #2 — a genuine contract disagreement between two sessions.** Note the asymmetry it leaves: `--set-prompt` validates only `args[i+1]` with `valid_value`, not the prompt text at `args[i+2]`, so `--set-prompt ses_a --dry-run` is still accepted (S9's `set_prompt_consumes_flag_looking_text` passes). Root must decide one rule and make the test and parser agree; do not simply delete the test.

All other observed suites were green at that snapshot: lib unit tests (4, including S5's new `ui.rs` tests), cli_edge_tests (17/18), api_failure_tests (6), age_tests (1), plus the new nudge/state/model/log/lock/json test binaries existed but were not all re-run after the latest edits. Re-run the full suite after the two blockers are fixed.

### 8.2 What each session actually changed (contract impact)

- **S7 `state.rs`:** made `Session.id` serde-defaulted; added `State::from_json` + private `normalize` (dedups session ids, clamps min_gap 0-3600, caps >=0, tick_interval 5-300, cooldown >=0, all timestamps >=0); `load_from_path` now logs non-NotFound read errors and uses a nanosecond corrupt-backup stamp; `save_to_path` now propagates serialization errors and writes to `tmp.<pid>` instead of `tmp`. **Impact:** `from_json` signature churn is the current build break (8.1). The normalization bounds match the GUI spinbutton ranges in `main.rs:1086-1142`, so no GUI conflict — but the dedup silently drops previously-duplicate ids and should be called out in CHANGELOG. No `Session` field was added, so R2/R3 from §4 did not materialize.
- **S7 `db.rs`:** `last_event` now wraps `json_extract` in `json_valid(data)`; `last_assistant` made `pub` with comment "callers can fetch timestamp+text in one query instead of two." **Integration gap:** no caller was updated — `main.rs:167` still calls `last_assistant_text` and `main.rs:282` still calls `last_assistant_ts` separately, so the stated optimization is unrealized. Purely additive otherwise; `main.rs` import still compiles.
- **S6 `nudge.rs`:** added `IO_LOCK: OnceLock<Mutex<()>>` and `append_and_rotate`, serializing history/log read-modify-write (backlog M2); `notify` now passes `--` before title; added `is_retryable_error` covering both `ureq` "status code 5xx/429" and the fallback path's `"HTTP 5xx/429"`; `fetch_models` now rejects non-2xx before parsing. **Impact:** all internal/behavioral — no public signature or payload changed, so `main.rs` and the GUI channels are unaffected. Good low-risk merge.
- **S6 `cli.rs`:** added `valid_value()` and applied it to `--export-state/--import-state`, `--pause/--resume`, `--kick`, `--set-model` (sid only); rewrote `HELP` as a multi-line literal. **Impact:** the `valid_value` rule is the direct cause of merge blocker #2. Help text still contains every flag asserted by S9 and README, so that part is safe.
- **S5 `ui.rs`:** added `status_tooltip(&Status)`; `format_age` switched to `saturating_sub`; CSS switched fixed hover/active hex to `shade()`, `.suggested-action`/`.destructive-action` to `button.`-qualified selectors, and `.summary`/`.note`/`.session-meta` to theme-aware colors; removed the `.scrolledwindow { background-color: #14181d; }` rule; added 4 unit tests. **Impact:** no signature change to existing helpers, so `main.rs` still compiles. Two root-side follow-ups: (a) `status_tooltip` is never called — `main.rs:379` still sets the badge tooltip to `v.reason`, so the accessibility improvement is dead code until root wires it; (b) removing the `.scrolledwindow` rule changes the GUI from a dark scroll area to the theme background — verify visually against S4's `UI_AUDIT.md` intent.
- **S9:** added nine new test binaries (cli_edge, state_merge, state_corrupt, api_failure, model_api, nudge_guard, log_rotation, lock_pid, json_status). They exercise only public library APIs and use `..Default::default()` in new `Session` literals, so they are resilient to field additions. The one failing test is the cross-session disagreement in 8.1.

### 8.3 Revised merge order for the landed diff

1. **Fix `state.rs` `from_json` consumer** (`Ok/Err` → `Some/None`, or change `from_json` back to `Option`) — restores compilation. S7 owns; root verifies.
2. **Resolve the `--resume --json` contract** — S6 and S9 agree on one rule (recommended: keep `valid_value` rejecting flag-looking ids and change S9's test to expect `Help`, because accepting `--json` as a session id is almost certainly a user error); update both in the same commit.
3. **Root patches to `main.rs` (no compile dependency, but required for the features to actually work):** wire `status_tooltip` into the badge at line 379; switch `last_assistant_text`/`last_assistant_ts` at lines 167 and 282 to the newly-public `last_assistant` to realize the one-query optimization; visually verify the CSS scroll-area change.
4. **Full `cargo test`** (target: baseline 18 binaries + 9 new = 27) must be green.
5. **Root final commit** with CHANGELOG/KNOWN_ISSUES/TEST_RESULTS/GLOBAL_TASK_BOARD updates; note the state-id dedup behavior change.

### 8.4 Additional checks the new tests imply

- `tests/state_merge_tests.rs::concurrent_saves_to_distinct_paths_are_independent` relies on the `tmp.<pid>` change; keep that change or the test's intent is invalidated. An old `.tmp` file left by a prior crash is no longer cleaned up — minor, note it.
- `tests/cli_edge_tests.rs::help_text_lists_the_documented_modes` pins the `HELP` literal — any future help-text edit must keep the six asserted flags.
- `tests/state_corrupt_tests.rs` should still pass with the nanosecond backup stamp (existing test checks only the filename prefix); verify after the `from_json` fix.
- The `IO_LOCK` mutex in `nudge.rs` serializes writes within one process only; the single-instance lock still allows a CLI process alongside the GUI to write history/log concurrently. Cross-process rotation races remain possible (known limitation worth a CHANGELOG note, not a blocker).
