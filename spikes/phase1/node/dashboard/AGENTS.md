# Purpose

Own the minimal Phase 1 dashboard-shell presentation assets used to prove that the dashboard remains a client of RELAY's existing structured command system.

# Ownership

- This file governs `spikes/phase1/node/dashboard/`.
- Parent Node and Phase 1 DOX contracts remain authoritative.
- Dashboard assets own presentation only; RELAY behavior stays in the shared command/core modules.

# Local Contracts

- Do not implement project, result, health, permission, or recovery business rules in browser JavaScript.
- Browser actions call the versioned structured dashboard execution adapter; the adapter forwards to the existing command dispatch.
- The Spike 6 dashboard is read-only and exposes only the command subset required to prove status, doctor, project-list, and result retrieval.
- Keep simple, detailed, and raw information progressively disclosed.
- Use plain language in the primary view; exact protocol/result JSON belongs behind Advanced/raw disclosure.
- No CDN, remote font, analytics, third-party script, or network dependency is permitted.
- Static assets must work with a strict same-origin Content Security Policy.

# Work Guidance

- Keep the shell small enough that framework cost remains measurable.
- Prefer semantic HTML and browser-native controls.
- Do not add dashboard-only capabilities.

# Verification

- Run Spike 6 integration tests.
- Verify the dashboard API returns the same structured command result semantics as CLI/host execution.
- Verify blocked dashboard commands do not become alternate command implementations.

# Child DOX Index

- No child DOX files currently exist under this folder.
