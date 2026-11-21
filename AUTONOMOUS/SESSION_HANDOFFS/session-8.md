# Session 8 Handoff — Security & Privacy (T008)

- **Role:** independent security and privacy reviewer, read-only.
- **Model:** opencode/hy4-flash-free#high (all work by this session; no other model used).
- **Repo:** /home/mint/session-keeper-rust, audited at commit `b9daf12`.
- **Status:** T008 COMPLETE (audit delivered; no source edits).

## Deliverables

- `AUTONOMOUS/SECURITY_FINDINGS.md` — rewritten from the placeholder into 10 evidence-backed findings with file:line and severity, plus verified non-issues and a local-attacker threat model summary.
- `AUTONOMOUS/SESSION_HANDOFFS/session-8-subagents.md` — worker delegation record.

## Result

1 High, 4 Medium, 3 Low, 1 Info.

- **H-01** service.json password is mode 0755 (world-readable) and the OpenCode API listens unauthenticated-reachable on 127.0.0.1:49374 — any local user can steal the credential and replay it. Highest-priority fix.
- **M-01** Cargo.lock pins libsqlite3-sys 0.28.0 with `bundled` (Cargo.toml:17) → covered by RUSTSEC-2026-0459 (MEDIUM). Not exploitable on this host because the DB/WAL files are owner-write-only; fix by upgrading rusqlite to 0.41 or linking system SQLite >= 3.54.0.
- **M-02** API base and credential-source path are env-overridable (config.rs:41-46) → credential exfiltration chain if the launcher environment is attacker-controlled; path overrides also permit file write/truncate.
- **M-03** raw `sid` interpolated into API URLs (nudge.rs:93-95, 240) → path/query injection via crafted or imported session id.
- **M-04** PID-based lock file (process.rs:3-18, main.rs:694-705) is spoofable and symlink-following → watchdog DoS / arbitrary small write.
- **L-01** state/history/log/audit/lock files are 0664 in a 0775 dir; prompts are stored in cleartext.
- **L-02** notify-send resolved via PATH; DB-sourced text can inject option flags (no shell injection — Command::arg used).
- **L-03** unvalidated state import; predictable `state.tmp` and arbitrary CLI export/import paths.
- **I-01** cleartext loopback Basic auth.

Verified non-issues: b64() correctness, ureq's `RedirectAuthHeaders::Never` default, no password in logs/errors/exports, parameterised SQL, no inbound listener.

## Root handoff / recommended next tasks

1. **S1 (root):** act on H-01 — chmod 600 service.json, chmod 700 its directory; consider restricting port 49374 to a Unix socket. This is a deployment/config change, outside this session's read-only scope.
2. **S6/S1:** apply M-03 URL-encoding of `sid` and M-04 flock-based locking (code changes — Session 8 cannot edit source).
3. **S6/S7:** set 0700/0600 modes on state/history/log/audit files and validate imported state (L-01, L-03).
4. **S1:** add `cargo audit` to CI and plan the rusqlite 0.41 upgrade (M-01) once the Rust toolchain is >= 1.89.
5. **S9:** add regression tests for `b64()`, sid validation, and file permission assertions if the above fixes land.
6. **S2/S3:** document the local-attacker threat model — this product has no network attack surface; all meaningful threats require local access or a compromised launcher environment.

## Blockers

- Subagent delegation was blocked by the runtime (`experimental.subagent_depth = 1`; this session is already depth 1). Verified worker count: **0**. All audits were performed directly. See `session-8-subagents.md`.
- `cargo audit` is not installed; RustSec data was consulted via web reference only.
