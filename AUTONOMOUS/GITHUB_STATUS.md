# GitHub Status

Repository: https://github.com/sahilwasgood/session-keeper-rust, created 2026-11-21.
Account assignment: sahilwasgood (active GitHub CLI account, verified with gh auth status).
Remote: origin HTTPS, default branch main. No account switching performed.

2026-11-22: Operator confirmed the project was intended as open source from the start. Repo visibility flipped PRIVATE -> PUBLIC after a secrets scan found no credentials in tracked files. Description and topics set (rust, gtk3, opencode, watchdog, linux, cli). MIT LICENSE, GitHub Actions CI (cargo test --all-targets + release build), and CONTRIBUTING.md added. CI passed on the first public push. Tag v0.1.0 and first GitHub release published.

Pre-public checks: tracked files contain no ghp_/github_pat_/sk- tokens, no private-key material, and no assigned credential literals. Config defaults are now portable (standard OpenCode paths, port 4096); this machine's custom paths live outside the repo in ~/.config/session-keeper/env.
2026-11-22: tagged and published v0.2.0 from main (31 test binaries green, CI passing); v0.1.0 remains as the initial public release.
2026-11-22: published v0.2.2 (persisted rolling nudge timestamps for the global cap).
2026-11-22: published v0.3.0 (configurable timestamp display, status filters, scrolling/layout fixes, CLI time preferences, F11 full screen).
