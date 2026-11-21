# Root subagent 9 — rusqlite upgrade assessment (RUSTSEC-2026-0459 / CVE-2026-119375)

Scope: `Cargo.toml`, `Cargo.lock`, API call sites in `src/db.rs`, `src/main.rs`, `tests/db_tests.rs`.
Read-only on the real repo — no source, manifest, lockfile, or test file was edited. Verification
was done in a disposable copy at `/tmp/opencode/rusqlite-check` (source/tests copied from the repo,
only that copy's `Cargo.toml` changed to `0.41.1`). Toolchain in place: `rustc/cargo 1.100.0`.

## BOTTOM LINE — exact recommendation

Change `Cargo.toml:17` to:

```toml
rusqlite = { version = "0.41.1", features = ["bundled"] }
```

Then run `cargo update -p rusqlite` (or just `cargo check`, which refreshes the lock).

This resolves to **rusqlite 0.41.1 → libsqlite3-sys 0.39.1 → bundled SQLite 3.54.1**, which is
outside the affected range (libsqlite3-sys 0.6.0–0.38.2 bundle SQLite 3.13.0–3.53.2) and satisfies
the ">= 3.54.0" requirement. It compiles and passes all tests on Rust 1.100.0. **No source changes
are required** — every API we use is source-compatible.

## WHY 0.41.1 (and not 0.31 / 0.40 / 0.43)

| rusqlite | libsqlite3-sys req | Bundled SQLite | MSRV | Notes |
|---|---|---|---|---|
| 0.31.0 (current) | ^0.28.0 → 0.28.0 | 3.43.x | — | AFFECTED |
| 0.40.2 | ^0.38.x | 3.53.x | 1.88 | AFFECTED (pre-fix) |
| **0.41.0** | **^0.39.0 → 0.39.1** | **3.54.0 / 3.54.1** | 1.88 (crate notes 1.89) | First fixed release (2026-10-24) |
| **0.41.1** | **^0.39.0 → 0.39.1** | **3.54.1** | 1.88/1.89 | Recommended: bugfix-only follow-up (2026-10-28) |
| 0.42.0 | ^0.40.0 → 0.40.0 | 3.54.1 | 1.89.0 | Also fine; explicit bundled bump |
| 0.43.0 | ^0.40.0 → 0.40.0 | 3.54.1 | 1.89.0 | Latest; adds empty-SQL rejection (see risk R2) |

Rationale: 0.41.1 is the smallest change that removes the advisory (single manifest line) and was
verified end-to-end. `version = "0.41.1"` is caret ^0.41.1, so it will not drift to 0.42/0.43.
If root prefers newest, **0.43.0** is also compatible (same APIs, MSRV 1.89) but introduces one
behavior change — see R2.

The advisory's `bundled-sqlcipher` caveat (0.39.0/0.39.1 with `sqlcipher-src 419.0.6` still bundles
3.53.4) does **not** apply: we enable `bundled`, not `bundled-sqlcipher`.

## API SOURCE COMPATIBILITY (0.31 → 0.41.1)

All call sites checked against docs.rs 0.41.1 and confirmed by a real compile of the project against
0.41.1:

- `Connection::open_with_flags` — unchanged signature (`src/main.rs:50`).
- `OpenFlags` / `OpenFlags::SQLITE_OPEN_READ_ONLY` — unchanged (`src/main.rs:3,50`).
- `Connection::busy_timeout(Duration::from_millis(...))` — unchanged (`src/main.rs:53`).
- `Connection::query_row(sql, params, closure)` — unchanged (`src/db.rs:14,30,60,69,84,92`;
  `src/main.rs:56`). `OptionalExtension` maps `QueryReturnedNoRows` to `None` exactly as before.
- `params!` macro — unchanged (`tests/db_tests.rs:30`).
- `OptionalExtension` trait and `.optional()` — present, `fn optional(self) -> Result<Option<T>>`
  (`src/db.rs:2,22,37,62,70,85,98`).
- `Row::get` / `r.get::<_, Option<String>>(0)` — unchanged.
- `bundled` feature name/semantics unchanged in 0.41/0.42/0.43.
- No `prepare`, `PRAGMA`, `column_names`, traced hooks, or `busy_handler` usage — so the 0.41
  PRAGMA-name validation and the statement-API changes in 0.41/0.43 do not touch us.
- `bitflags` is already 2.13.2 in the current lockfile, so no major-version dep bump is introduced.

## VERIFICATION ACTUALLY PERFORMED (Rust 1.100.0)

In `/tmp/opencode/rusqlite-check`:

- `cargo check --all-targets` → `Finished dev profile`, no errors. Only warning: pre-existing
  unused import `Write` in `tests/timeout_tests.rs:1` (unrelated).
- `cargo test` → **160 passed, 0 failed** across all test binaries + doc-tests.
- Runtime version probe (`rusqlite::version()`) → `SQLITE_VERSION=3.54.1`.
- Resulting scratch lockfile pins `rusqlite 0.41.1` (checksum
  `da9bba6f3506135e120a6586df05be18b48d18896600646a4bec8f993d91a594`) and
  `libsqlite3-sys 0.39.1` (checksum
  `14a5f01b87b92705d07b433c06f96cbc764637d81a0bffb5734ce17cf0f9a2b3`).

Note for root: an early scratch compile surfaced `state.rs:118-120` "expected Option, found Result"
errors. That was a stale snapshot — another agent was mid-edit changing `State::from_json` from
`Option` to `Result` while I copied source. After refreshing the scratch source those errors vanished;
they are NOT caused by the rusqlite upgrade and do not appear in the 160-test green run.

## EXACT COMMANDS FOR ROOT (after editing Cargo.toml)

```bash
cd /home/mint/session-keeper-rust
cargo update -p rusqlite
cargo check --all-targets
cargo test
grep -A2 '^name = "libsqlite3-sys"' Cargo.lock      # expect version = "0.39.1"
cargo tree -p rusqlite                              # expect libsqlite3-sys v0.39.1
cargo audit                                         # if cargo-audit installed — expect RUSTSEC-2026-0459 gone
# optional runtime proof in a throwaway example:
printf 'fn main(){println!("{}", rusqlite::version());}\n' > examples/sqlite_ver.rs 2>/dev/null || \
  { mkdir -p examples && printf 'fn main(){println!("{}", rusqlite::version());}\n' > examples/sqlite_ver.rs; }
cargo run --quiet --example sqlite_ver               # expect 3.54.1
```

Expected lockfile delta: `rusqlite 0.31.0 → 0.41.1`, `libsqlite3-sys 0.28.0 → 0.39.1` (minor
transitive churn only; hashlink/cc/pkg-config/vcpkg already present).

## RISKS / NOTES

- **R1 (low, residual):** the advisory is local/info-disclosure and only bites when opening a DB
  plus a sibling `-wal` that a less-trusted party could write. We open the user's own session DB
  read-only. Fixing it anyway is correct for audit cleanliness.
- **R2 (only if choosing 0.43.0):** 0.43.0 rejects empty or comment-only SQL (`#1956`). Our queries
  are all real SQL, but if any future code passes an empty statement, behavior changes from "no-op"
  to `Err(Error::Empty)`. 0.41.1 avoids this.
- **R3:** if anyone later enables `bundled-sqlcipher`, re-check the advisory — 0.39.x SQLCipher
  still bundles vulnerable SQLite 3.53.4.
- I did not run `cargo audit` (not confirmed installed) and did not test 0.42.0/0.43.0 builds —
  only the recommended 0.41.1 was compiled and tested.

## SOURCES

- https://rustsec.org/advisories/RUSTSEC-2026-0459.html (authoritative advisory + patched versions/MSRV)
- https://rustsec.debian.net/advisories/RUSTSEC-2026-0459.html (same text, independent mirror)
- https://github.com/rusqlite/rusqlite/releases/tag/v0.41.0 (`Bump bundled SQLite version to 3.54.0 #1913`; MSRV 1.88/1.89)
- https://github.com/rusqlite/rusqlite/releases/tag/v0.41.1 (bugfix-only)
- https://github.com/rusqlite/rusqlite/releases/tag/v0.42.0 (`Bump bundled SQLite version to 3.54.1 #1920`)
- https://github.com/rusqlite/rusqlite/releases/tag/v0.43.0 (empty-SQL rejection `#1956`, UAF fixes)
- https://crates.io/api/v1/crates/rusqlite/versions?per_page=10 (0.41–0.43 rust-version/edition)
- https://crates.io/api/v1/crates/rusqlite/0.41.0/dependencies (`libsqlite3-sys ^0.39.0`)
- https://crates.io/api/v1/crates/libsqlite3-sys/versions?per_page=12 (0.39.0/0.39.1/0.40.0 MSRV 1.88)
- https://docs.rs/rusqlite/0.41.1/rusqlite/struct.Connection.html
- https://docs.rs/rusqlite/0.41.1/rusqlite/trait.OptionalExtension.html
- https://docs.rs/rusqlite/0.41.1/rusqlite/macro.params.html

Confidence: **HIGH** — patched version mapping from the advisory and upstream release notes, API
presence from docs.rs 0.41.1, and a real compile + 160/160 tests + runtime `3.54.1` probe on the
exact installed Rust 1.100.0 toolchain.
