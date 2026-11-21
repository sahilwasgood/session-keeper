# Root subagent 6 — configuration & state-schema compatibility audit

Scope: `src/state.rs`, `src/config.rs`, `src/cli.rs`, `README.md`, `AUTONOMOUS/*`, plus the
state/import/export call sites in `src/main.rs`. Read-only audit; no source, test, or real-state
files were modified. Runtime checks used throwaway files under `/tmp/opencode/` and the current
`target/debug` binary (21:51 build). Note: `target/release` is stale (21:22, older than
`state.rs` 21:45) — do not use it to validate current behavior.

## VERIFIED OK

- **Serde default coverage — complete.** All 15 `State` fields (`state.rs:49-81`) and all 15
  `Session` fields (`state.rs:15-47`) used by `main.rs` carry `#[serde(default)]` or
  `#[serde(default = "fn")]`. There are no `#[serde(skip)]` fields, so a well-formed export
  round-trips every field. `Default for State` matches the serde defaults field-for-field.
- **Old / partial state files load safely.** Importing a JSON object with only `min_gap`,
  `tick_interval_s`, and `sessions` produced a fully populated state with correct defaults for
  every missing field (verified at runtime). `State::load_from_path` also backs up malformed JSON
  and returns defaults (`state.rs:108-130`).
- **Env override names — consistent.** All 11 names documented in `README.md:40` exactly match
  the names read in `config.rs:24-56` (`SESSION_KEEPER_DB/DRIVE/STATE/HISTORY/LOCK/AUDIT/LOG/
  API/SERVICE_JSON/DRY_RUN/NOTIFY`). No other source file reads a differently spelled variant.
  The direct `HOME` reads at `main.rs:1212,1229` use the same fallback as `config.rs:10-12`.
- **CLI import/export round-trip (well-formed input) — field-complete.** `ExportState` writes
  `to_string_pretty` of a loaded State; `ImportState` deserializes it and saves. No fields are
  dropped. `tests/state_tests.rs::save_then_load_preserves_session_fields` covers the main ones.

## CONCRETE DEFECTS

### D1 (High) — Both import paths bypass normalisation; bad values are persisted to state.json
`state.rs:132` claims `from_json` is the "Single parse + sanitize entry point shared by load and
import." It is not. Only `State::load_from_path` (`state.rs:118`) calls it. Both importers use raw
deserialization:

- CLI: `main.rs:775` — `serde_json::from_str::<State>(&text)`
- GUI: `main.rs:1231-1233` — `std::fs::read_to_string(...).and_then(|s| serde_json::from_str::<State>(&s).ok())`

So imports skip `normalize()` (`state.rs:137-160`): no clamping of `min_gap`,
`tick_interval_s`, caps or cooldowns, no `.max(0)` on counters/timestamps, no removal of empty
session IDs, no deduplication of session IDs.

Runtime proof (current debug binary, temp state + temp lock only):
import `{min_gap:-50, tick_interval_s:1, sessions:[ses_a/-5, ses_a/9, ""/1]}` wrote state.json
containing exactly `min_gap:-50`, `tick_interval_s:1`, three sessions including the duplicate
`ses_a` and the empty-ID entry, and `nudges:-5`. A subsequent normal load silently normalised
it to one `ses_a` with `min_gap 0`, `nudges 0` — i.e. the on-disk state is unvalidated until some
other code path reloads and re-saves it. Negative counters feed `nudge.rs` cap checks; duplicate
IDs make tick updates and GUI edits hit only the first entry.

Fix: route both imports through `State::from_json(&text)`; make GUI import report the same precise
error as CLI (`main.rs:778`) and back up state before overwrite. Add a test asserting import of
partial/negative/duplicate input yields the same result as `State::from_json`.

### D2 (Medium) — Documented `SESSION_KEEPER_DRY_RUN` env override is overwritten at startup (real nudges possible)
`main.rs:666-679`:

- unless `--dry-run` was passed, `SESSION_KEEPER_DRY_RUN` is forcibly set from
  `boot_state.persist_dry_run` (`main.rs:667-672`);
- `SESSION_KEEPER_NOTIFY` is always forcibly set from `boot_state.persist_notifications`
  (`main.rs:676-679`).

`Config::dry_run()` / `Config::notifications()` (`config.rs:47-56`) therefore never see the
operator's launch-time env values when a state file exists. Concretely: `SESSION_KEEPER_DRY_RUN=1`
with saved `persist_dry_run=false` becomes `"0"` and `tick()` sends **real** nudges
(`main.rs:205`). `PRODUCT_REQUIREMENTS.md:229` already logs the NOTIFY half as P2; the dry-run
half is not recorded anywhere and is a safety-relevant precedence bug.

Fix: only `set_var` when the env var is absent, or define env > persisted > built-in precedence
and document it in `README.md:38-40`.

### D3 (Low) — GUI import silently discards invalid files and keeps a separate, non-overridable path
`main.rs:1231-1243` uses `.ok()` twice and only shows "Import failed: no valid export file"; no
reason, no backup, no normalisation (see D1). The GUI settings export/import path is hard-coded to
`$HOME/session-keeper-export.json` (`main.rs:1212-1213, 1229-1230`) and ignores
`SESSION_KEEPER_STATE`, so GUI "Import settings" and CLI `--import-state` are not interchangeable
without a manual copy. Already tracked as `R-014` (P1) in `PRODUCT_REQUIREMENTS.md:270` — audit
confirms it.

### D4 (Low) — `Session::default()` disagrees with serde's missing-field default for `enabled`
`Session` derives `Default` (`state.rs:15`), giving `enabled = false` (bool zero value), while a
missing `enabled` field deserializes via `default_enabled()` to `true` (`state.rs:6,19`). Current
`main.rs` constructs `Session` explicitly (`main.rs:1029-1032`), so no live bug, but any future
`Session::default()`-based creation flips the semantics. Low-priority consistency note.

## Coverage gaps (recommended, not defects)

- No test exercises `State::from_json` with missing fields, negative values, duplicate IDs or
  empty IDs; `state_tests.rs` only covers defaults-on-missing-file, save/load and corrupt backup.
- No test asserts that CLI/GUI import uses the same normalisation as load (the gap D1 exploits).
- No test covers `SESSION_KEEPER_DRY_RUN`/`SESSION_KEEPER_NOTIFY` precedence against persisted
  settings. `config_tests.rs` only covers direct `Config::*` reads.

## Skipped / risk

Did not run the full `cargo test` suite (read-only audit; runtime checks used the built binary and
temp files only). Did not open the GUI, so D3's runtime path is confirmed by code only. The main
risk to the operator is D1: importing a hand-edited or older state file can persist negative
counters and duplicate session IDs, and D2: an env-level dry-run request can be silently cancelled
at startup.
