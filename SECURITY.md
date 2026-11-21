# Security policy

## Credentials

Session Keeper never stores the OpenCode server password in its own state, history, or log files. It reads the password at runtime from OpenCode's `service.json` and uses it only for loopback API requests. Keep that file readable only by your user account; on filesystems that ignore permission bits, move it to one that enforces them.

The stored state (`~/.config/session-keeper/state.json`) contains session IDs, prompts, and counters. It does not contain tokens or passwords. The audit export includes settings, history, and the decision log; review it before sharing.

## Reporting a vulnerability

Open a private security advisory in this repository rather than a public issue. Include a description, reproduction steps, and the affected version. Do not post exploit details or credentials in a public issue.

## Threat model

This is a local, single-user tool. It opens the OpenCode database read-only and sends nudges to a loopback API. It is not designed to run as a service, accept remote input, or manage secrets for other users. The single-instance lock prevents two watchers from acting on the same sessions.
