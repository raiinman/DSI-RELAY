# Phase 2 Start Here

## Status

Phase 2 - Core and command system - is the active next implementation phase after Phase 1 stack selection closed on 2026-09-25.

## Locked foundation

Do not reopen Phase 1 stack choices without contradictory evidence. Current selections are Rust + bundled SQLite, the shared JSON command registry, one per-user daemon with CLI and embedded dashboard surfaces, out-of-process adapters with AppContainer/LPAC isolation on qualified Windows builds, signed side-by-side Windows updates, foreground-safe scheduling, watcher-plus-reconciliation indexing, scoped evidence reduction, and bounded structured JSONL diagnostics with optional ETW deep tracing.

## Phase 2 goal

Turn the proven spike contracts into the first coherent core implementation without adding UEFN product behavior prematurely.

Build in this order:

1. establish a production-shaped core crate/layout from the selected Rust prototype
2. promote project registry, command registry, jobs, result envelopes, persistent result store, and machine-mode CLI contracts
3. add permission/effect classes, idempotency, transaction foundations, usage metrics, provenance/trust metadata, and data-classification/egress policy primitives
4. add agent/client identity attribution and credential-handle boundaries without exposing secrets to command/model context
5. promote the adapter broker/manifest lifecycle and strong Windows worker isolation behind stable interfaces
6. preserve one command/business-logic surface for CLI, dashboard, gateways, adapters, and AI clients
7. add focused migration tests proving Phase 1 fixture/state compatibility where applicable

## Rules

- Preserve D-146 onward history and D-153 through D-159 selections.
- Do not copy spike-only shortcuts into production merely because they benchmarked well.
- Keep deterministic local work outside AI calls.
- Keep project-specific and UEFN-specific behavior behind later adapters.
- Every promoted subsystem needs failure/restart behavior, schema/version implications, security/privacy implications, tests, and a measured resource check.
- Keep machine output structured and non-interactive.

## First implementation slice

Start with the smallest production-shaped vertical that can prove one project registration, one structured command dispatch, one durable result, one job/checkpoint, status/doctor health, and restart persistence through the selected Rust/SQLite/registry/diagnostic stack. Do not add new product features until that slice is coherent and tested.

Phase 1 evidence remains under `spikes/phase1/results/`; D-159 is the final Phase 1 stack-selection decision.
