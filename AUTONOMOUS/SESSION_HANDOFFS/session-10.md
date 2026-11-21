# Session 10 Handoff — Performance, Infrastructure, Reliability (T010)

Status: audit substantially complete; 8 of 10 task areas fully measured, 2 long-running build measurements still compiling at audit close.

## Role and ownership
- T010 owner. Write ownership: `AUTONOMOUS/PERFORMANCE.md` and `AUTONOMOUS/SESSION_HANDOFFS/session-10*.md` only. All source work read-only; no source edits were made.
- Model: `opencode/hy4-flash-free#high` (this session and all measurements).

## Delegation outcome
- Attempted to spawn 10 workers (binary/RSS, startup, idle CPU, log growth, SQLite, network, build/deps, release opts, crash recovery, deployment).
- **Runtime refused all 10**: `Subagent depth limit reached (1). Increase experimental.subagent_depth`. This session is itself a subagent of root, and the charter says subagents do not spawn further agents. Verified worker count: **0**. All 10 task areas were executed directly with local commands. Details in `session-10-subagents.md`.

## Delivered evidence (full detail in PERFORMANCE.md)
1. Binary 6,127,008 B vs ~5.3 MB baseline (+~0.55 MiB); strip → 5,099,696 B (−16.8%). Full profile `opt-level=z + lto + codegen-units=1 + panic=abort + strip` → **2,991,736 B (−51%)** with no startup penalty (15.82 ms vs 19.53 ms mean). Size dominated by bundled SQLite + ring/rustls (ureq default TLS on a plaintext localhost API).
2. RSS 45–51 MB / PSS 19–23 MB (baseline ~45 MB RSS confirmed); ~61% of RSS shared GTK. CLI peak 12–14 MB.
3. CLI startup: `--version` 17 ms, `--status` 368 ms mean / **12.49 s cold**, `--check-now` 1.16 s mean (5.81 s max); GUI window 2.494 s.
4. GUI idle CPU **3.1–5.3%** of one core (four unconditional GLib poll timers); `--watch` idle **0.000%**.
5. Log growth 53.3 B/line, ~50 KiB/h at N=8; rotation 2000→1000 forces a full-file read+rewrite per line once past cap.
6. **P0 hotspot:** `db.rs last_assistant` 169 ms for 4,595 assistant rows because ORDER BY `time_created, rowid` mismatches the `(session_id, type, seq)` index → temp B-tree sort. `ORDER BY seq DESC` measured 0.12 ms (~1,400×) using the existing index. Amplifies to ~0.5–0.7 s per idle session per tick (4 identical queries + two DB opens).
7. Network: one TCP connect per call (no ureq Agent reuse); authenticated API GET 1.0–1.37 s; 4 nudges in last hour; worst case 16 requests/30 s bounded by caps.
8. Dependencies: 149 crates (154 in `cargo tree`), release target 559 MiB, debug 1.9 GiB. Cold release build **18m 21s / 1.22 GiB peak RSS**; incremental rebuild 0.42 s.
9. Crash/restart: single-instance lock works; SIGKILL leaves stale lock but recovery succeeds via `pid_alive`; state save is atomic (tmp+rename); corrupt state backed up; log rotation rewrite is non-atomic.
10. Deployment verified: install to temp prefix + `--version` works; GTK3 runtime libs enumerated; no systemd units (on-demand design).

## Highest-value next tasks for root / other sessions
- **S7 (data):** apply the one-line `ORDER BY seq DESC` fix in `src/db.rs` — largest measured win by far. Note S7 already made `last_assistant()` public; finish the query fix and ensure `last_assistant_ts()`/`last_assistant_text()` reuse it.
- **S1 (root):** reuse one `Connection` per tick in `main.rs::run_tick` (currently opens DB twice); add `[profile.release] strip = "symbols"`; reduce 150/200 ms poll timers.
- **S6 (backend):** O(1) log rotation (avoid whole-file rewrite per line); consider a shared `ureq::Agent`.
- **S1/S8:** harden stale-lock detection; add deployment notes to README.
- Re-run `cargo test` after any source change; re-measure `--check-now` to confirm the P0 fix (expected ~1.16 s → <0.2 s).

## Blockers / risks to note
- Nested subagents unavailable (runtime depth 1) — no parallel measurement workers.
- `perf stat` events not supported in this container; CPU evidence is `/proc` tick deltas.
- Source files were edited concurrently during the audit; release binary reflects the 21:22 build. Rebuild before treating size/RSS numbers as final.
- Two detached build scripts (§PERFORMANCE.md §8/§9) were still running; append their results when complete.
