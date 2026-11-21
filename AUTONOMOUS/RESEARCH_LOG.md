# Research Log

2026-11-21: four TinyFish research passes, 172 source URLs total.
- sk-research-alternatives.md: nudge/watchdog projects, debounce, loop detection, caps, dry run.
- sk-research-ui.md: GTK3 CSS + freedesktop symbolic icons recommended; avoid egui/iced redraw cost.
- sk-research-ux.md: 10 UX rules; highest value are question skip, debounce, reason logs, receipts, pause switches.
- sk-research-backend.md: sync worker + mpsc, ureq/rusqlite, atomic saves, jittered retry, CLI and env.

2026-11-21: fresh code review found Critical C1/C2 and Important I1-I5; all fixed in commit ecef291 and follow-ups.

## 2026-11-21 — T002 research intelligence pass (session 2)

Method / provenance: research performed directly by session 2 with TinyFish (`search` then
`fetch_content`) via `execute`. Web content treated as untrusted data; only primary sources
(official docs, specs, crates.io/RustSec APIs, repo docs, arXiv) used for claims. ~20 searches
and ~18 full-page fetches; ~90 unique URLs surfaced, ~25 authoritative pages read in full.
Every finding below has a source URL. Unsupported claims are marked UNVERIFIED.

### Execution blocker (important)
Ten `subagent` spawns were attempted with the required model `opencode/hy4-flash-free#high`.
All ten were rejected by the runtime: `Subagent depth limit reached (1)`. Session 2 is itself a
subagent, so **0 workers were created** — the verified maximum available was 0. Model ID was
confirmed active with the `high` variant via `opencode.models`. Research was therefore done
directly; no agent counts are fabricated. See SESSION_HANDOFFS/session-2-subagents.md.

### R1 — Watchdog / nudge alternatives
- `xanstomper/watchdog-agent` — monitors opencode/MCP sessions, detects stalled window, types a
  recovery prompt (`continue`/`y`/`proceed`), then verifies the session actually resumed (CPU
  normal) before escalating. https://github.com/xanstomper/watchdog-agent
- `asheshgoplani/agent-deck` watchdog — in-tree since v1.7.63; takes a **SHA-256 hash of tmux
  pane output** each safety-poll tick to detect a child sitting `waiting` with unchanged
  output; per-session cap 3 restarts/300 s and global cascade guard 1 restart/60 s; Telegram
  escalation; 62-test suite. https://raw.githubusercontent.com/asheshgoplani/agent-deck/main/documentation/WATCHDOG.md
- `DraconDev/opencode-auto-continue` — stall recovery (abort + continue), todo nudging, review
  on completion, question auto-answer, hallucination-loop detection, prompt/concurrency guard.
  Documented pitfall: when a guard blocks `sendContinue()`, recovery is lost permanently.
  https://github.com/DraconDev/opencode-auto-continue
- `Mte90/opencode-auto-resume` — stall/streaming/dead-stream recovery with `continuePrompt`
  default `"continue"`. https://github.com/Mte90/opencode-auto-resume
- Other comparators: `SnowAIGirl/claude-tmux-dog` https://github.com/SnowAIGirl/claude-tmux-dog ;
  `RyanSam` macOS menu-bar auto-continue https://ryansam.com/agent-auto-continue/ .
- Why watchdogs matter: OpenCode issue #57349 — tool-call markup written as plain text ends the
  step with `finish=stop`, no tool part, call silently lost, session goes idle.
  https://github.com/anomalyco/opencode/issues/57349 ; issue #59635 — service restart auto-resumes
  suspended sessions and runs pending tasks without consent.
  https://github.com/anomalyco/opencode/issues/59635 ; issue #52049 — a 45 s event-stream idle
  watchdog restarts the service and aborts sessions (a watchdog that harms).
  https://github.com/anomalyco/opencode/issues/52049
- **Recommendation:** our hash-based unchanged-output detection, per-session + global caps and
  human escalation match proven designs. Add a *post-nudge verification* step (did the session
  actually resume? if not, alert rather than nudge again). Do **not** copy question auto-answer:
  it conflicts with the consent problem in #59635. Cap global restarts/nudges to avoid #52049.

### R2 — OpenCode API capabilities
- `opencode serve [--port 4096] [--hostname 127.0.0.1] [--cors <origin>]`; basic auth via
  `OPENCODE_SERVER_PASSWORD` (user defaults to `opencode`); OpenAPI 3.1 spec at `/doc`.
  Global endpoints: `GET /global/health`, `GET /global/event` (SSE).
  https://dev.opencode.ai/docs/server
- HTTP API has 173 operations / 518 schemas. `GET /api/info` (`server.info`), `POST /api/pair`,
  `GET /auth/connect/{code}`, `GET /api/location`, `POST /api/location/reload` (cancels pending
  permissions/forms; running sessions continue at next step boundary — confirms a
  pending-permission concept exists). https://opencode.ai/docs/api
- SDK: `OpenCode.create()`, `sessions.create`, `sessions.prompt({sessionID, text})`,
  `events.subscribe()` AsyncIterable. https://opencode.ai/docs/sdk
  v2 client equivalents: `client.session.create` / `client.session.prompt`.
  https://opencode.ai/v2/docs/build/client
- Exact session-status endpoint and a documented `question`/permission-request state were NOT
  confirmed in the pages read — marked UNVERIFIED; check `openapi.json`
  (https://opencode.ai/openapi.json) before switching transports. DB schema is not documented in
  official docs fetched.
- **Recommendation:** keep read-only DB polling as the detection path (no auth/HTTP dependency,
  works beside the TUI). Keep nudges abstracted behind one function so an optional HTTP
  `session.prompt` transport can be added later; do not write to OpenCode's DB.

### R3 — GTK3 accessibility and focus
- `gtk_window_present()` docs explicitly warn it is called too late to gather a valid timestamp
  for focus-stealing prevention. https://docs.gtk.org/gtk3/method.Window.present.html
- GtkWindow supports an `<initial-focus>` UI-definition element naming the widget focused on map.
  https://docs.gtk.org/gtk3/class.Window.html
- GTK3 builder UI supports `<accessibility>` with `<action>` and `<relation type="labelled-by"/>`;
  `GtkAccessible` is a thin wrapper over `AtkObject` (AT-SPI is the screen-reader path).
  https://docs.gtk.org/gtk3/class.Widget.html , https://docs.gtk.org/gtk3/class.Accessible.html
- GTK4 removed ATK in favour of a new a11y stack; staying on GTK3 keeps the mature ATK path.
  https://lists.gnome.org/archives/commits-list/2020-July/msg08637.html
- Raising/focus etiquette: https://discourse.gnome.org/t/how-to-raise-a-window-in-gtk3-i-e-unminimize-it-bring-it-on-top-of-other-windows-and-focus-it/17100
  GNOME guidance: notifications should not be intrusive or distracting.
  https://developer.gnome.org/documentation/tutorials/notifications.html
- **Recommendation:** give every session card/button an accessible name, define `<initial-focus>`,
  use a timestamped present only on explicit user action, and never auto-focus the window for an
  alert — let the notification carry the alert.

### R4 — Rust GUI / icon state in 2026
- Primary crates.io metadata: `gtk` latest 0.19.0 (updated 2026-09-08), `glib` 0.22.11
  (2026-11-14), `rusqlite` 0.40.2, `ureq` 3.4.2, `libsqlite3-sys` 0.38.2.
  https://crates.io/api/v1/crates/gtk , .../glib , .../rusqlite , .../ureq , .../libsqlite3-sys
- gtk-rs GTK3 is maintained again: repo https://github.com/gtk-rs/gtk3-rs ; the prior
  "unmaintained" advisory was **withdrawn** (see R9). `relm4` is gtk4-rs-based v0.11.0
  https://crates.io/crates/relm4 ; `egui` is at 0.34.2 https://crates.io/crates/egui .
- **Recommendation:** stay on GTK3 + freedesktop symbolic icons. A migration to GTK4/relm4,
  iced or egui is a rewrite with no payoff for a small on-demand window. A bounded bump to
  gtk 0.19 / glib 0.22 is worth a separate compatibility branch, not this cycle.

### R5 — Linux notification delivery
- Desktop Notifications Spec 1.2: https://specifications.freedesktop.org/notification/1.2 ;
  hints are optional and unknown hints must be ignored. Standard hints include `urgency` (BYTE
  0 low / 1 normal / 2 critical), `category`, `desktop-entry`, `image-path`, `action-icons`.
  https://specifications.freedesktop.org/notification/latest/hints.html
- Notifications are delivered to a notification *server* over D-Bus; with no daemon running
  `notify-send` has no destination — the GUI must degrade gracefully (in-window badge/log).
  GNOME HIG: notifications must not be intrusive/distracting.
  https://developer.gnome.org/documentation/tutorials/notifications.html
- **Recommendation:** keep `notify-send` (zero new dependencies); map "session needs you" to
  urgency critical and routine info to normal; set `desktop-entry`; keep cooldown/dedupe; on
  nonzero exit fall back to the in-window attention state. Verify exact `notify-send` flags from
  the local man page during implementation (NOT sourced here — UNVERIFIED detail).

### R6 — State file and SQLite concurrency
- WAL lets readers keep reading the original DB while writers append to the WAL; readers pin the
  last valid commit ("end mark"). https://www.sqlite.org/wal.html ; snapshot isolation:
  https://www.sqlite.org/isolation.html
- Read-only WAL caveats: a read-only connection cannot create WAL/-shm files; WAL and -shm must
  sit beside the DB; older SQLite required write access to read a WAL-mode DB.
  https://sqlite.org/forum/info/8debe446d2b5dbe3 , https://hynek.me/til/sqlite-read-only-wal-locked/
- `busy_timeout` does not cover a lock upgrade from a DEFERRED transaction; prefer declaring
  connections read-only. https://github.com/CIRISAI/CIRISPersist/issues/1238 ,
  https://berthub.eu/articles/posts/a-brief-post-on-sqlite3-database-locked-despite-timeout/
- rusqlite `Connection` / `OpenFlags::SQLITE_OPEN_READ_ONLY`:
  https://docs.rs/rusqlite/latest/rusqlite/struct.Connection.html
- `flock` beats pidfiles: kernel releases the lock on process death and avoids stale-lock races.
  https://stackoverflow.com/questions/220525/ensure-a-single-instance-of-an-application-in-linux ,
  https://blog.petrzemek.net/2017/07/24/ensuring-that-a-linux-program-is-running-at-most-once-by-using-abstract-sockets
- **Recommendation:** keep `SQLITE_OPEN_READ_ONLY` + `busy_timeout`, never `immutable=1`; keep the
  `flock` single-instance lock; use temp-file + fsync + rename for state.json (practitioner
  standard, UNVERIFIED in this pass — verify before relying on it). A single-instance lock alone
  does not stop GUI and CLI double-nudging in sequence — add a shared "last nudge" timestamp.

### R7 — LLM-agent failure modes a watchdog should detect
- Infinite Agentic Loops study: 6,549 repos scanned, 68 confirmed IAL failures in 47 projects
  (91.9% precision); consequences are cost exhaustion, model DoS, context growth and repeated
  side effects. https://arxiv.org/html/2607.01641v1
- Progress-reporting study: premature completion stops 1.6%, missing-progress-marker stops 22.7%
  — do not trust an agent's own "done" claim. https://arxiv.org/html/2609.08589v1
- Sentry: "stalls" are agents that recognise the objective but fail to convert it into a concrete
  next step (plan without action). https://arxiv.org/html/2610.02994v1
- ClayBuddy failure taxonomy (underspecification etc.): https://arxiv.org/html/2606.19380v3
- Concrete OpenCode stall signature from issue #57349: assistant message with `parts: []` and all
  token counts 0 after `step-finish`. https://github.com/anomalyco/opencode/issues/57349
- **Recommendation:** hash-based unchanged-output loop detection is well supported (also used by
  agent-deck). Keep two-check debounce and min-gap because long thinking/compilation are real
  false-trigger risks. If the DB exposes empty-parts/zero-token tail messages, add that as a
  second stall signal. Keep caps as the defence against loop amplification.

### R8 — UX of automation trust
- CHI 2023 survey: trust must be calibrated to actual system capability; over- and under-trust
  are both failure modes. https://dl.acm.org/doi/10.1145/3544548.3581197
- NIST human-centred automation/AI: trustworthiness via accuracy, reliability, explainability.
  https://www.nist.gov/programs-projects/human-centered-si ,
  https://www.nist.gov/publications/trust-and-artificial-intelligence-draft
- Automation-bias review: over-reliance means users stop critically evaluating output.
  https://link.springer.com/doi/10.1007/s00146-025-02422-7
- Calibration as momentary trust judgement: https://pmc.ncbi.nlm.nih.gov/articles/PMC11573890
- **Recommendation:** our dry run, reason codes, audit export, pause/resume switches and
  notification cooldown are the right trust primitives. Highest-value next surfacing: "why did it
  nudge" next to each nudge in the GUI, and a single global pause/kill control always visible.

### R9 — Licensing and dependency security (highest-priority findings)
- Primary crate licenses (crates.io API): `gtk` MIT, `glib` MIT, `rusqlite` MIT,
  `libsqlite3-sys` MIT, `ureq` MIT OR Apache-2.0 (URLs as in R4). Native C libraries: GTK3/GLib
  are LGPL-2.1 and SQLite is public domain (standard knowledge; primary licence texts NOT fetched
  in this pass — UNVERIFIED for distribution/legal decisions).
- **Correction:** RUSTSEC-2024-0417 ("gtk-rs GTK3 bindings no longer maintained") is **WITHDRAWN —
  "should be ignored"**; the repository was unarchived and development resumed.
  https://rustsec.org/advisories/RUSTSEC-2024-0417.html
- **Actionable:** RUSTSEC-2026-0459 / CVE-2026-119375 affects `libsqlite3-sys` 0.6.0–0.38.2
  (bundled SQLite < 3.54.0): crafted DB + WAL with mismatched page size can make queries return
  uninitialized page-cache memory. Fixed in `libsqlite3-sys` 0.39.0 / `rusqlite` 0.41 (Rust >=
  1.89). Our `Cargo.lock` pins **libsqlite3-sys 0.28.0** — in range. Exposure is local and needs a
  crafted DB/WAL from a less-trusted party, so severity is moderate for our own-DB read path.
  https://api.osv.dev/v1/vulns/RUSTSEC-2026-0459 ,
  https://rustsec.org/advisories/RUSTSEC-2026-0459.html
- Tooling: RustSec/cargo-audit https://rustsec.org/ ; cargo-deny license/ban policy
  https://github.com/EmbarkStudios/cargo-deny ; OSV https://osv.dev/ .
- **Recommendation:** bump `rusqlite` to >=0.41 (or link a system SQLite >=3.54 instead of
  `bundled`), run `cargo audit` in CI, add `cargo-deny` for a license allow-list, and record LGPL
  obligations if distributing a statically linked binary. Flag the bundled-SQLite advisory to
  session 8 (security) and session 10 (infra).

### R10 — Curated real-world sources to monitor
- Primary issues: OpenCode #57349, #59635, #52049 (URLs in R1); oh-my-openagent #4484
  https://github.com/code-yeongyu/oh-my-openagent/issues/4484
- Primary project docs: agent-deck WATCHDOG.md, opencode-auto-continue, opencode-auto-resume,
  watchdog-agent (URLs in R1)
- Primary research: arXiv 2607.01641, 2609.08589, 2610.02994 (URLs in R7)
- Primary specs/docs: freedesktop notifications, GTK docs, RustSec, crates.io (URLs in R3/R5/R9)
- Secondary/anecdotal: r/opencode auto-continue plugin thread
  https://www.reddit.com/r/opencode/comments/1svw0yi/made_a_tiny_opencode_plugin_that_autocontinues/ ;
  HN OpenCode threads https://news.ycombinator.com/item?id=47460525 ,
  https://news.ycombinator.com/item?id=48978112
- **Recommendation:** monitor OpenCode GitHub issues, opencode-auto-continue, and the RustSec
  feed; treat Reddit/HN as idea sources only.

### Consolidated next tasks for the organisation
1. (S8/S10) Resolve RUSTSEC-2026-0459: bump rusqlite >=0.41 or drop `bundled`; verify Rust >=1.89.
2. (S3/S6) Add post-nudge verification and shared last-nudge timestamp (GUI/CLI double-nudge).
3. (S5) Add accessible names + `<initial-focus>`; never auto-focus on alerts.
4. (S6) Keep notify-send with urgency hints and no-daemon fallback; verify local man-page flags.
5. (S1/S7) Verify exact OpenCode session-status / permission-state endpoints in openapi.json
   before any HTTP transport work; keep DB read path in the meantime.

2026-11-22: verified /api/session/{id}/permission and /form response shape {"data":[...]}; used for precise human-input detection instead of text-only question heuristic.
