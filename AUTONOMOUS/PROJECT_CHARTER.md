# Session Keeper - Project Charter

Product: a lightweight on-demand Rust GTK3 GUI + CLI watchdog for OpenCode sessions. It watches sessions in the OpenCode database, nudges idle ones with a configurable prompt, keeps guards against false triggers, and alerts a human when needed.

Runtime: OpenCode2 only. Root model: opencode/hy4-flash-free, variant high. All orchestrators and subagents must use the same model and variant.

Organisation: 10 orchestrator sessions. Session 1 is root. Sessions 2 to 10 own distinct workstreams. Each orchestrator manages up to 10 subagents; subagents do not spawn further agents.

Operating loop: research, analyse, delegate, implement, integrate, test, inspect, audit, improve, repeat.

Ownership:
- Session 1 root: integration, src/main.rs, Cargo.toml, final commits, AUTONOMOUS root records.
- Session 2 research: read-only research, RESEARCH_LOG.md.
- Session 3 product: requirements matrix, PRODUCT_REQUIREMENTS.md.
- Session 4 UI/UX: design/accessibility audit, UI_AUDIT.md, no source edits.
- Session 5 frontend: src/ui.rs only.
- Session 6 backend/CLI: src/nudge.rs, src/cli.rs.
- Session 7 data: src/state.rs, src/db.rs.
- Session 8 security: read-only audit, SECURITY_FINDINGS.md.
- Session 9 QA: tests/** only.
- Session 10 performance/infra: PERFORMANCE.md, build/deployment notes, read-only elsewhere.

No agent may edit a file outside its ownership. Changes to owned files must keep `cargo test` green.
