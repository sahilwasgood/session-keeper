# Performance, Infrastructure and Reliability — Session 10 (T010)

Audit date: 2026-11-21. Method: local measurement only (`cargo`, `ps`, `/proc`, `/usr/bin/time`, `strace`, `perf`, `ldd`, `size`, `nm`, `python3 sqlite3`, `curl`, `xwininfo`). All numbers below are measured, with commands shown. No source file was edited.

**Caveat — moving target:** other sessions edited source during this audit. `src/state.rs` 21:45, `src/db.rs` 21:48, `src/ui.rs` 21:51, `src/nudge.rs` 22:01, `src/cli.rs` 22:05. The release binary measured here was built 21:22:44, so binary/RSS/startup numbers describe that build. Source-level findings describe the newer files. The `last_assistant` ORDER BY hotspot is still present in the current `src/db.rs`.

## 1. Executive summary — measured vs baseline

| Metric | Baseline | Measured | Delta / status |
|---|---|---|---|
| Release binary | ~5.3 MB | **6,127,008 B (5.84 MiB)** | ~+0.55 MiB |
| Stripped binary | — | **5,099,696 B (4.86 MiB)** | `strip` saves 1,027,312 B (16.8%) |
| GUI RSS | ~45 MB | **45,204–51,208 kB** (PSS 19,541–23,242 kB) | matches baseline; ~61% of RSS is shared GTK mapping |
| CLI peak RSS | — | 12.2–13.8 MB | small |
| CLI `--version` | — | 0.017 s mean | fine |
| CLI `--status` | — | 0.368 s mean; **12.49 s cold** | cold DB penalty is severe |
| CLI `--check-now` | — | 1.163 s mean (max 5.81 s) | SQLite hotspot, see §6 |
| GUI window mapped | — | **2.494 s** | acceptable |
| GUI idle CPU | — | **3.1–5.3% of one core** | higher than expected for idle |
| `--watch` idle CPU | — | **0.000%** | ideal |

## 2. Binary size and resident memory

Commands: `stat -c %s`, `size`, `nm --size-sort -S`, `strip` on a scratch copy, `ldd`, `grep smaps_rollup`.

```
size target/release/session-keeper-rust
   text 4,982,392   data 112,200   bss 4,864   dec 5,099,456
top symbols: zmij STATIC_DATA (0x4300), sqlite3VdbeExec (0xbfdb),
             ecp_nistz256_precomputed (0x25000), ring k25519/curve25519 tables,
             session_keeper_rust::main (0x92f2), sqlite3WhereCodeOneLoopStart,
             sqlite3Pragma, sqlite3Select
ldd: 68 shared libs, 3,440,811 B (3.28 MiB) total on disk
strip: 6,127,008 -> 5,099,696 B
```

- Binary grew ~0.55 MiB over the 5.3 MB baseline. Dominant static contributors: **bundled SQLite** (`rusqlite` `bundled` feature) and **ring/rustls** TLS tables pulled in by `ureq` default features (`tls` → `rustls` → `ring`, `webpki-roots`). Neither is used on the hot path (API is plaintext localhost HTTP).
- Live GUI (pid 109083, 40+ min old): `Rss 45,204–51,208 kB`, `Pss 19,541–23,242 kB`, `Shared_Clean 31,436 kB`, `Private_Dirty 16,380 kB`, `Swap 4,604 kB`. VSZ 541,368 kB. The ~45 MB RSS baseline is almost entirely shared GTK libs; **true private cost is ~19–23 MB PSS**. Swap usage suggests the long-lived process pages out some cold GTK pages.
- CLI peak RSS (`/usr/bin/time -v` equivalent, `%M`): `--version` 12,164 kB, `--status` 13,732 kB, `--check-now` 13,756 kB.

## 3. Startup time

10 runs each, isolated env, dry-run (`/usr/bin/time -f '%e %U %S'`, mean/min/max):

| Mode | mean | min | max |
|---|---|---|---|
| `--version` | 0.017 s | 0.010 | 0.030 |
| `--help` | 0.009 s | 0.000 | 0.010 |
| `--status` | 0.368 s | 0.150 | 0.710 |
| `--status --json` | 0.271 s | 0.180 | 0.380 |
| `--check-now` | 1.163 s | 0.410 | **5.810** |

- Cold vs warm after `echo 3 > /proc/sys/vm/drop_caches`: `--status` **12.49 s cold → 0.53 s warm**. The 1.06 GiB DB must be faulted from disk; `open_db()`'s probe query and the per-session classification walk bring in a large fraction of it.
- GUI: **2.494 s** from spawn to window mapped (`xwininfo -root -tree` polling, 50 ms resolution, `DISPLAY=:0.0`, isolated state/lock).
- `strace -c -f --status` (598 syscalls): `mmap` 310 (58.7% of syscall time), `openat` 79, `read` 118, `close` 79, `newfstatat` 12. Startup cost is dominated by dynamic-loader/GTK mapping and DB I/O, not app logic.

## 4. CPU while idle

`/proc/<pid>/stat` utime+stime deltas over three 20 s windows on the live GUI:

```
window 1: 62 ticks / 20 s => 3.10% of one core
window 2: 72 ticks / 20 s => 3.60%
window 3: 106 ticks / 20 s => 5.30%   (concurrent cargo builds active)
--watch (interval forced to 300 s, dry run): 0 ticks / 20 s => 0.000%
```

`perf stat -p` was blocked (`<not supported> cpu-cycles:u`) — perf events unavailable in this container; `/proc` deltas are the evidence.

Cause: four GLib poll timers run unconditionally on the UI thread — one 150 ms channel receive, three 200 ms channel receives, plus a 5 s state watch and 10 s heartbeat. Each `try_recv` on an empty channel is a cheap but non-zero wakeup; ~23 wakeups/s keep the GUI at a few percent CPU. `--watch` uses a blocking `thread::sleep`, hence 0%.

## 5. Log growth and I/O amplification

Measured file statistics:

```
session-keeper.log  1,422 lines, mean 53.3 B/line (49–68), 75,725 B total
history.jsonl         205 lines, mean 101.2 B/line (87–289), 20,749 B total
state.json            8 watched sessions, tick_interval_s=30
```

`log_line()` and `append_history()` in `nudge.rs` each: open-append + write one line, then **read the entire file**, call `rotate_text(text, 2000, 1000)`, and if over 2000 lines **rewrite the whole file**. Evidence: `strace -c` on one `--check-now` shows 87 `openat`, 120 `read`, 85 `close`, 3 `write`, 319 total syscalls — dominated by reads of the whole files.

Growth math (30 s ticks, measured 53.3 B/line):

| Watched | Lines/sess/tick | Log bytes/hour | History bytes/hour* |
|---|---|---|---|
| 8 | 1 | 50 KiB/h | 95 KiB/h |
| 8 | 2 | 100 KiB/h | 190 KiB/h |
| 20 | 2 | 250 KiB/h | 474 KiB/h |
| 50 | 2 | 625 KiB/h | 1.16 MiB/h |

\* assuming one history entry per emitted nudge. With N=8 at 1 line/session/tick the 2000-line cap is hit in **~2.1 h**; after that *every log line rewrites ~1,000 lines (~53 KiB)*. Steady-state file size is capped at 1,000 lines (~53 KiB log, ~101 KiB history) — observed log 76 KiB/1,422 lines is consistent. Risk is I/O amplification and a non-atomic full rewrite window, not unbounded disk growth.

## 6. SQLite read cost — P0 hotspot

Measured with `python3 sqlite3` on the live 1.06 GiB DB (read-only, warm cache; 64,335 `session_message` rows, 986 `session_v2` rows):

```
last_event (classify)     0.13 ms
last_assistant (classify) 169.44 ms   <-- hotspot
session_exists            0.01 ms
session_model / title     0.01 ms
last_idle_ts              0.51 ms
open_db probe             0.01 ms
SELECT count(*) FROM session_message 1,210 ms (full scan, not used by app)
```

Per-session `last_assistant` cost scales with assistant-message count:

| session (truncated) | assistant rows | cost |
|---|---|---|
| ses_eeb40c… | 4,595 | 169.3 ms |
| ses_ed0a97… | 7,215 | 174.9 ms |
| ses_e9c3a7… | 6,033 | 50.6 ms |
| ses_ecb713… | 1,522 | 17.5 ms |
| ses_e31829… | 1,069 | 6.8 ms |

**Root cause (EXPLAIN QUERY PLAN):**
```
SEARCH session_message USING INDEX session_message_session_type_seq_idx
  (session_id=? AND type=?)
USE TEMP B-TREE FOR ORDER BY
```
The query orders by `time_created DESC, rowid DESC`, but the only matching index is `(session_id, type, seq)`. SQLite sorts every assistant row of the session on every call.

**Proven fix (no index creation needed):** change ORDER BY to `seq DESC`:
```
SEARCH session_message USING INDEX session_message_session_type_seq_idx (session_id=? AND type=?)
cost = 0.12 ms  (≈1,400x faster)
```
`db.rs` still uses the slow ORDER BY in current source.

**Amplification per tick:** `tick()` calls `classify()` (1× `last_assistant` for idle sessions) and `fingerprint(&last_assistant_text(...))` (2nd call), then `run_tick()` calls `classify()` again for the views plus `last_assistant_ts()` (4th call). An idle session with 4,595 assistant messages therefore costs up to **~680 ms of SQLite per tick**, which explains `--check-now` 1.16 s mean / 5.81 s max. Also note `run_tick()` opens the DB **twice** per tick (`main.rs` tick + views sections).

## 7. Network call frequency

- Call sites: `send_nudge()` (POST `/api/session/{sid}/prompt`, 10 s timeout, up to 2 attempts on retryable errors), `fetch_models()` (GET `/api/model`, 15 s timeout), `switch_model()` (POST, 10 s timeout).
- `ureq` 2.12.1 is called through free functions (`ureq::post`/`ureq::get`), so **no Agent/connection reuse** — `strace -e connect` on `--models` shows **1 TCP connect**. Measured authenticated API latency: `/api/model` **1.00 s** (402,912 B), `/api/session` **1.37 s** (27,051 B); without auth both return 401 in 0.17–1.34 s.
- `ureq` default features pull `rustls` + `ring` + `webpki-roots` into the binary (§2) despite the configured API being plaintext `http://127.0.0.1:49374`.
- Observed rate: 4 nudge entries in the last hour of `history.jsonl` (205 total rows, 202 `nudge`, 3 `hourly-cap`). Worst case with 30 s ticks, 8 sessions, all due: up to 8 nudges/tick × 2 attempts = 16 HTTP requests per 30 s, bounded by `min_gap=10 s`, per-session cap (0 = unlimited here) and `global_cap=60/h`.
- Model dialog fetches the full 403 kB model list on **every** Model button click; `model_cache` caches only the result for display, not the fetch.

## 8. Build time and dependency footprint

- `Cargo.lock`: **149 packages** from 6 direct dependencies (`gtk`, `glib`, `rusqlite[bundled]`, `serde`, `serde_json`, `ureq`). Heavy families: GTK stack (gtk/gdk/gio/glib/pango/cairo/atk + sys crates), bundled SQLite (`libsqlite3-sys`), and TLS stack (`rustls`, `ring`, `webpki-roots`, `icu_*`, `url`).
- Disk: `target/release` **559 MiB**, `target/debug` **1.9 GiB**.
- Measured cold release build in isolated `CARGO_TARGET_DIR` (4 CPUs, i3-4150):
```
Finished `release` profile [optimized] target(s) in 18m 21s
COLD_RELEASE real=1102.04 s user=511.43 s sys=29.96 s maxrss=1,284,264 KB (~1.22 GiB)
artifact: 6,143,848 B  (slightly larger than the 21:22 binary — source changed mid-audit)
incremental no-op rebuild: real=0.42 s  (0.36 s cargo)
cargo tree crate count: 154 (Cargo.lock has 149 packages)
```
The long pole is C compilation of bundled SQLite (`libsqlite3-sys`) and `ring`, plus linking the GTK wrappers. Cold builds need ~18 min and ~1.2 GiB RAM; incremental rebuilds are sub-second.

## 9. Release build optimisation options

`Cargo.toml` has **no `[profile.release]`**. Measured directly on the existing binary:
- `strip` on a scratch copy: 6,127,008 → 5,099,696 B (**−1.03 MB, −16.8%**) with no behavioural change. This is the single cheapest win and puts the binary below the 5.3 MB baseline.
- Symbol analysis shows the size is dominated by bundled SQLite and ring/rustls static tables, so `opt-level="z"` / `lto` would mainly affect those.
- Measured profile A/B on a snapshot copy of the project (isolated target dirs), against a same-snapshot default cold build of **6,143,848 B**:

| Profile | Binary | vs default | `--version` mean (30 runs) |
|---|---|---|---|
| default (no `[profile.release]`) | 6,143,848 B | — | 19.53 ms |
| A: `strip = "symbols"` | **5,111,928 B** | −16.8% (−1,031,920 B) | 16.14 ms |
| B: `opt-level="z", lto=true, codegen-units=1, panic="abort", strip="symbols"` | **2,991,736 B** | **−51.3% (−3,152,112 B)** | 15.82 ms |

Variant B also built faster than the default cold build (9m 06s vs 18m 21s) because its snapshot reused warm crate artifacts for the first profile. Both variants start marginally *faster* than the baseline (smaller page-in). B is a 2.85 MiB binary — roughly half the current size — at the cost of full-LTO build time on a truly cold tree and `panic="abort"` (no unwinding). `strip` alone is the zero-risk one-line win; B is the maximum-size win if S1 accepts the profile change.

## 10. Crash / restart recovery — measured

```
# live keeper PID in lock file
$ echo 109083 > isolated.lock; keeper --check-now
Session Keeper is already running (pid 109083).     <- single-instance OK

# dead PID: proceeds; clean exit removes lock
$ echo 999999 > isolated.lock; keeper --check-now  -> ran, lock removed (LockGuard::drop)

# SIGKILL mid-watch
$ kill -9 <watch pid>; lock still contains that pid (stale)
$ keeper --check-now                                 -> proceeds, pid_alive() saw no cmdline, lock cleaned
```

- `pid_alive()` checks `/proc/<pid>/cmdline` contains `session-keeper-rust`. A stale lock is therefore recovered, but the check is weak: *any* process whose cmdline contains that substring could block a restart, and a recycled PID running a different command is correctly treated as dead. No stale-lock age check exists.
- `State::save()` is **atomic**: writes `state.json.tmp.<pid>` then `fs::rename` (`state.rs:170-172`). No leftover tmp files were observed.
- Corrupt state is backed up to `state.json.corrupt-<ns>` and defaults are loaded (`state.rs:125-126`).
- Log/history rotation rewrites are **not** atomic (full `fs::write`) — a crash mid-rewrite can truncate either file. Rotation keeps the newest 1,000 lines, so loss is bounded.

## 11. Deployment / release instructions (verified)

No systemd units exist (`find /etc/systemd -iname '*session*keeper*'` → 0); the design is explicitly on-demand. Verified install:

```
cargo build --release
install -m 755 target/release/session-keeper-rust ~/.local/bin/session-keeper-rust
~/.local/bin/session-keeper-rust --version     # -> "Session Keeper 0.1.0 (Rust)"
```

- Runtime prerequisites: GTK3 shared libraries — `libgtk-3.so.0`, `libgdk-3.so.0`, `libglib-2.0.so.0`, `libgobject-2.0.so.0`, `libpangocairo-1.0.so.0`, `libpango-1.0.so.0`, `libcairo.so.2`, `libgdk_pixbuf-2.0.so.0`, `libatk-1.0.so.0`, `libepoxy.so.0`, plus X11/Wayland libs (68 libs, 3.28 MiB total). `ldd target/release/session-keeper-rust` for the full list.
- Hardcoded defaults in `src/config.rs`: DB `/media/mint/WDC_WD10EZRX_932G/opencode/data/opencode.db` (must be readable — 1.06 GiB), drive `/media/mint/WDC_WD10EZRX_932G`, API `http://127.0.0.1:49374`, password from `.../opencode/config/service.json`. All overridable via the `SESSION_KEEPER_*` env vars listed in README.
- State/history/log live in `~/.config/session-keeper/` and `~/session-keeper.log`. Single-instance lock at `~/.config/session-keeper/keeper.lock` (read-only DB, no service needed).
- CLI quick reference is in README; `--check-now` and `--status` work headless, GUI needs `DISPLAY`.

## 12. Prioritised recommendations (proposals only — no source edits made)

1. **P0 — `db.rs last_assistant`: change `ORDER BY time_created DESC, rowid DESC` to `ORDER BY seq DESC`.** Measured 169 ms → 0.12 ms using the existing index. Removes ~0.5–0.7 s per idle session per tick and most of `--check-now` variance. One-line change; owner S7.
2. **P1 — reuse one `Connection` per tick** and pass it to both `tick()` and the views loop; `run_tick` currently opens the 1 GiB DB twice. Owner S1/main.rs.
3. **P1 — stop re-querying the same assistant row** in `classify()` / `last_assistant_text()` / `last_assistant_ts()` / views; the new public `last_assistant()` (timestamp+text in one query, already added by S7) should be the single source. Up to 4 identical 169 ms queries per idle session per tick today.
4. **P2 — add `[profile.release] strip = "symbols"`** to `Cargo.toml`: −1.03 MB, no runtime cost. Owner S1.
5. **P2 — reduce idle timer frequency** (150 ms / 3×200 ms → 500 ms–1 s, or arm only when a channel has work) to cut GUI idle CPU from 3–5% to <1%. Owner S1/main.rs.
6. **P2 — make log rotation O(1) per line** (append-only with size-triggered rotate, or track line count) instead of read+rewrite the whole file on every line. Owner S6/nudge.rs.
7. **P3 — reuse a single `ureq::Agent`** for localhost calls (connection reuse; removes TLS/rustls if HTTP-only via `default-features=false, features=["gzip"]`), shrinking binary and avoiding ~1 s per model fetch. Owner S1/Cargo.toml + S6.
8. **P3 — strengthen stale-lock handling** (`pid_alive` + lock mtime) and document the on-demand deployment in README. Owner S1/S8.

## Measurement status at audit close

All §8 and §9 numbers are final (cold build, A/B profile builds and startup re-measurement all completed). No worker subagents could be spawned: the runtime enforces `experimental.subagent_depth = 1` (this session is itself a subagent), and the charter states subagents do not spawn further agents. Verified worker count: **0**; all tasks were executed directly.
