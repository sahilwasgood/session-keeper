# Global Task Board

Statuses: BACKLOG, READY, ASSIGNED, IN_PROGRESS, BLOCKED, REVIEW, TESTING, COMPLETED, REJECTED.

| ID | Task | Priority | Owner | Status | Evidence |
|---|---|---|---|---|---|
| T001 | Root stabilisation after review fixes | Critical | S1 | COMPLETED | cargo test 18 binaries, commit b9daf12 |
| T002 | Research current alternatives and watchdog UX | High | S2 | COMPLETED | pending |
| T003 | Product requirements and missing workflows | High | S3 | COMPLETED | pending |
| T004 | UI/UX and accessibility audit | High | S4 | COMPLETED | pending |
| T005 | Improve src/ui.rs helpers and CSS safely | Medium | S5 | COMPLETED | pending |
| T006 | Backend and CLI robustness audit | High | S6 | COMPLETED | pending |
| T007 | State/database integrity and migration audit | High | S7 | COMPLETED | pending |
| T008 | Security and privacy audit | High | S8 | COMPLETED | pending |
| T009 | Expand tests for edge cases and workflows | High | S9 | COMPLETED | pending |
| T010 | Performance, build and reliability audit | Medium | S10 | COMPLETED | pending |
| T011 | Root integration of approved changes | High | S1 | IN_PROGRESS | continuous |

All T002-T010 completed by orchestrators directly (nested subagents blocked by runtime depth limit). Root integrated verified changes; see AUTONOMOUS/SESSION_HANDOFFS/.
