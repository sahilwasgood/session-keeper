# Security Findings — Session 8 (T008) independent review

Scope: read-only audit of `session-keeper-rust` at commit `b9daf12`. All findings below are evidence-backed with file:line references and were confirmed against the running host where relevant. Speculation is excluded; verified non-issues are listed at the end so they are not re-audited.

Severity summary: **1 High, 4 Medium, 3 Low, 1 Info**.

---

## H-01 — OpenCode Basic-auth password is world-readable and the API is reachable by every local user (HIGH)

- `/media/mint/WDC_WD10EZRX_932G/opencode/config/service.json` is mode **0755**, owner `mint:mint`; its parent `config/` is also 0755. The file contains a `password` key (verified without printing the value).
- `src/config.rs:57-63` — `Config::password()` reads that file fresh on every call and returns the password; `src/config.rs:44-46` locates it.
- `src/nudge.rs:139`, `src/nudge.rs:216`, `src/nudge.rs:239` — Base64-encode `opencode:<password>` into the `Authorization` header.
- `src/config.rs:5` sets the base URL to `http://127.0.0.1:49374`; `ss -ltnp` confirms `opencode.exe` (pid 15790) listening on `127.0.0.1:49374`.

**Impact:** any local account can read the credential and replay it against the loopback API to read/modify sessions — full API-level compromise from an unprivileged local account. The Rust code's in-process secret handling is clean (see verified non-issues); the exposure is the file mode and the unauthenticated reachability of the endpoint, which is exactly where the "secret handling from service.json" and "local API trust boundary" audits land.

**Remediation:** `chmod 600` on service.json and `chmod 700` on its directory; restrict the API to a Unix socket or otherwise enforce per-user access to loopback port 49374.

---

## M-01 — Bundled SQLite in libsqlite3-sys 0.28.0 is covered by RUSTSEC-2026-0459 (MEDIUM; not currently exploitable on this host)

- `Cargo.toml:17` — `rusqlite = { version = "0.31", features = ["bundled"] }`; `Cargo.lock:625-627` pins `libsqlite3-sys` **0.28.0**.
- RustSec advisory **RUSTSEC-2026-0459** (issued 2026-11-09, CVSS 4.0 = 6.8 MEDIUM): `libsqlite3-sys` 0.6.0–0.38.2 built with the `bundled` feature ships SQLite < 3.54.0, which can return uninitialized page-cache memory when a DB is opened alongside a sibling `-wal` whose page size differs → local information disclosure. Fixed in `libsqlite3-sys` 0.39.0/0.39.1 (SQLite 3.54.0/3.54.1), released with `rusqlite` 0.41.
- Exploit precondition is a DB + WAL writable by a less-trusted party. On this host `opencode.db`, `opencode.db-wal`, `opencode.db-shm` are mode **0755** (owner-write only), so a non-owner cannot craft the pair — the advisory is not reachable in the current deployment. It is still a real, version-specific dependency exposure.
- No git/path dependencies: all **149** `Cargo.lock` packages are crates.io with checksums. No ureq advisory was found for pinned 2.12.1. `cargo audit` is not installed locally, so no full scan was run.

**Remediation:** upgrade to `rusqlite >= 0.41` / `libsqlite3-sys >= 0.39.1` (requires Rust 1.89), or drop `bundled` and link system SQLite >= 3.54.0. Add `cargo audit` to CI.

---

## M-02 — API endpoint and credential source are fully environment-overridable: credential-exfiltration and file-write chain (MEDIUM, conditional on env control)

- `src/config.rs:41-43` — `api_base()` from `SESSION_KEEPER_API`; `src/config.rs:44-46` — `services_json()` from `SESSION_KEEPER_SERVICE_JSON`; `src/config.rs:57-63` — `password()` reads that path.
- `src/nudge.rs:139`, `src/nudge.rs:216`, `src/nudge.rs:239` attach the Basic header to whatever host `api` names, with no allowlist.
- If an attacker can set the process environment (wrapper script, systemd `Environment=`, privileged launcher), they can point `SERVICE_JSON` at any JSON file with a `password` field and `API` at a host they control — the app Base64s that value and sends it to the attacker on every `models`/`kick`/nudge. The same override family (`SESSION_KEEPER_LOG`, `HISTORY`, `AUDIT`, `DB`, `DRIVE`, `STATE`, `LOCK`) redirects file operations; e.g. `SESSION_KEEPER_AUDIT` reaches `std::fs::write` at `src/main.rs:632`, truncating any file the user can write.
- Note: `src/main.rs:667-679` deliberately rewrites `SESSION_KEEPER_DRY_RUN` and `SESSION_KEEPER_NOTIFY` from persisted state, neutralising those two overrides; all path/API overrides remain live.

**Remediation:** validate `api_base()` against a loopback allowlist and refuse to attach credentials to non-loopback hosts; ignore path overrides outside an allowed root in any privileged launch context.

---

## M-03 — Session id interpolated into API URLs without encoding (MEDIUM)

- `src/nudge.rs:93-95` — `prompt_url()` uses `format!("{}/api/session/{}/prompt", api, sid)` with the raw `sid`. `src/nudge.rs:240` — `switch_model()` does the same for the `/model` path.
- `sid` enters from CLI args (`src/cli.rs:77-85`, `src/cli.rs:94-106`), from imported state (`src/main.rs:767-788`), or from the GUI add box whose only check is `sid.starts_with("ses_")` plus a DB existence lookup (`src/main.rs:1021-1022`). State import bypasses even that check.
- A crafted id containing `/`, `?`, `#`, or `%2e%2e` can rewrite the request path/query, potentially reaching a different API endpoint with the attached credentials.

**Remediation:** percent-encode `sid` at URL construction, and validate `^ses_[A-Za-z0-9_-]+$` when a session is added or imported.

---

## M-04 — Single-instance lock is a spoofable, symlink-following PID file (MEDIUM/LOW)

- `src/process.rs:3-12` — `pid_alive()` only checks that `/proc/<pid>/cmdline` contains the substring `session-keeper-rust`. `src/process.rs:14-18` — `read_lock()` parses any integer.
- `src/main.rs:694-705` — a live PID that is not self causes startup to abort. Any local user can write a fake PID pointing at a process whose cmdline contains the string (e.g. `exec -a session-keeper-rust sleep 1000`) for a reliable watchdog DoS. There is no `/proc/<pid>/stat` start-time check, so PID reuse also causes false positives.
- `std::fs::write(&lock_path, ...)` at `src/main.rs:701` follows symlinks and races the read at `src/main.rs:695` (TOCTOU). An attacker with write access to the config dir can pre-place a symlink and get the PID digits written to an arbitrary file. On-disk `keeper.lock` is mode **0664**.

**Remediation:** replace the PID file with `flock(2)` on a 0600 descriptor. If retaining a PID file, validate start time and create with `OpenOptions::create_new(true).mode(0o600)`.

---

## L-01 — Sensitive state/history/log/audit files are group-writable and world-readable (LOW/MEDIUM)

- Verified modes: config dir **0775**; `state.json`, `history.jsonl`, `keeper.lock` **0664**; `session-keeper.log`, `session-keeper-audit.txt` **0664**; `opencode.db*` **0755**.
- No explicit permissions are set anywhere: `src/state.rs:131-138` (save), `src/nudge.rs:155-176` (history), `src/nudge.rs:186-205` (log), `src/main.rs:624-634` (audit), `src/main.rs:701` (lock), `src/main.rs:1215` (GUI export). All use plain `std::fs::write` / `OpenOptions` and inherit the umask.
- Contents include full per-session prompt text (`src/nudge.rs:164-165` stores the prompt in history, `src/main.rs:627` dumps the whole `State` into the audit file), session ids, and API error strings. Group `mint` includes `sudo`, `adm`, `users`, `sambashare`, so the 0664 group-write bit lets any process in those groups tamper with state/lock/history; other local users can read the contents.

**Remediation:** create the config dir 0700 and files 0600 via `DirBuilderExt::mode()` / `OpenOptionsExt::mode()`; consider storing a hash instead of full prompt text in history.

---

## L-02 — notify-send resolved via PATH; option injection via DB-sourced text (LOW/MEDIUM)

- `src/nudge.rs:51-61` — `Command::new("notify-send")` resolves through `PATH`. An attacker controlling `PATH` or a writable directory earlier in it can substitute a binary that runs as the keeper user (requires env control or a writable PATH entry).
- No shell is invoked (`Command::arg`), so there is **no shell command injection**. However `title`/`message` derive from session titles in the DB (`src/db.rs:81-87`, used at `src/main.rs:110`, `src/main.rs:128`, `src/main.rs:143`, `src/main.rs:197`, `src/main.rs:231`) and may begin with `-`, which notify-send can parse as an option.

**Remediation:** invoke an absolute path (e.g. `/usr/bin/notify-send`) and pass `--` before the title/message arguments.

---

## L-03 — Unvalidated state import and predictable temp/export paths (LOW)

- `src/main.rs:767-788` and GUI import `src/main.rs:1228-1243` deserialize arbitrary JSON into `State` with no validation (`src/state.rs:48-80` supplies only serde defaults). Downstream effects: unbounded prompt length sent to the API (`src/main.rs:211-214`); negative/zero timing values — `min_gap < 0` and `alert_cooldown_s < 0` effectively disable throttles (only `tick_interval_s` is clamped at `src/main.rs:645` and `src/main.rs:791`); a multi-GB JSON file can exhaust memory during parse.
- `src/state.rs:135-137` writes to the fixed sibling `state.tmp` then renames; `std::fs::write` follows a pre-placed symlink. `src/cli.rs:41-53` accepts arbitrary `--export-state` / `--import-state` paths and `src/main.rs:758`, `src/main.rs:768` use `std::fs::write` / `read_to_string`, following symlinks → arbitrary file read/overwrite as the invoking user. Relevant only where a less-privileged actor can influence argv or the config directory.

**Remediation:** validate imported fields (numeric bounds, id regex, prompt length); create temp files with `create_new(true)` + 0600; canonicalise and confine export/import paths.

---

## I-01 — Basic credentials travel in cleartext over loopback (INFO)

- `src/config.rs:5`, `src/nudge.rs:139`: credentials are sent over plain HTTP on `127.0.0.1`. Any local user with CAP_NET_RAW can sniff loopback (the `mint` account is in `sudo`, so effectively available). Given H-01 the password is already readable, so this adds little on this host.

**Remediation:** use a Unix socket or TLS for the API.

---

## Verified non-issues (do not re-audit)

- `src/nudge.rs:13-26` `b64()` is a correct RFC 4648 Base64 encoder including padding for input lengths mod 3 of 0, 1, and 2. The `Basic opencode:<password>` format is correct.
- ureq **2.12.1** defaults `RedirectAuthHeaders::Never` (verified in local crate source `/home/mint/.cargo/registry/src/.../ureq-2.12.1/src/agent.rs:22-31,262-263`), so the Authorization header is dropped on redirects — no cross-host credential leak.
- The Rust code never prints or writes the password to logs, audit, state, or export files. `src/nudge.rs:147` maps ureq errors with `format!("{}", e)`; those strings do not contain the Authorization header.
- All SQL in `src/db.rs` is parameterised with `?1` — no SQL injection.
- The product opens no listening socket (outbound HTTP client only); the only listener on 49374 belongs to OpenCode itself.

## Method / limitations

- Read-only: no source or AUTONOMOUS file was modified except this report and the two session-8 handoffs.
- 10 worker subagents were requested but the runtime enforces `experimental.subagent_depth = 1` and this session is itself a child of root, so **0 workers were spawned**; all ten assigned audits plus the local-attacker threat model were performed directly by this session. See `SESSION_HANDOFFS/session-8-subagents.md`.
- Web/RustSec data was used only as reference for M-01 and treated as untrusted; no instructions from web content were followed.
- **Post-audit change note:** other sessions edited `src/cli.rs`, `src/db.rs`, `src/nudge.rs`, `src/state.rs` and `src/ui.rs` concurrently with this review. All file:line references above are to the tree as read during the audit (baseline commit `b9daf12`), so a few line numbers have since shifted. One concurrent edit is security-relevant: `src/state.rs` now writes to a PID-suffixed `state.tmp.<pid>` instead of the fixed `state.tmp`, which narrows but does not close L-03's symlink issue (the name is still predictable from the victim PID and the write still follows symlinks). No other concurrent edit addressed a finding in this report.
