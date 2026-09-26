# Phase 2 first slice

Status: Complete — accepted on 2026-09-25. Evidence: `PHASE2_FIRST_SLICE_EVIDENCE.md`.

Goal: promote the proven Phase 1 contracts into one production-shaped core vertical before adding new product features.

The first slice must prove, together:
- one per-user Rust host
- one shared command registry
- one project registration
- one durable result ID
- one durable job/checkpoint
- structured machine-mode execution with no prompts
- status and doctor health
- restart persistence
- provenance/trust fields on durable output
- diagnostics health independent from operational storage health

Acceptance:
1. one command path serves CLI and host clients;
2. validation happens before business logic;
3. result and error envelopes are versioned;
4. restart preserves project/result/job state;
5. duplicate-safe/idempotency metadata has a defined storage boundary;
6. no UEFN-specific behavior enters the core;
7. tests cover malformed input, damaged storage, restart, and incompatible versions;
8. resource checks remain within the Phase 1 baseline order of magnitude.

Phase 1 spike code is evidence to promote deliberately, not production source to copy wholesale.
