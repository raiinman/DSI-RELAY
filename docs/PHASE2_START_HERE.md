# Phase 2 Start Here

## Status

Phase 2 - Core and command system - is complete.

The three production-shaped Core slices are complete and accepted. The formal closure review passes every published Phase 2 exit criterion; see `PHASE2_CLOSURE_REVIEW.md`.

- Slice 1 promotes the per-user Rust daemon/CLI transport, shared command registry, schema-2 SQLite project/result/job/idempotency state, provenance/trust fields, diagnostics health, restart persistence, and damaged-store behavior. Evidence: `PHASE2_FIRST_SLICE_EVIDENCE.md`.
- Slice 2 promotes permission/effect enforcement, trusted actor/client/delegator attribution, project scoping, schema-3 transaction/usage/credential/egress state, data-classification/local-only egress primitives, durable credential-handle metadata/revocation, restart persistence, and migration from schema 1/2. Evidence: `PHASE2_SECOND_SLICE_EVIDENCE.md`.
- Slice 3 promotes the generic adapter manifest/broker lifecycle, registry-bound capabilities, artifact/component integrity revalidation, bounded failure/quarantine behavior, and the qualified stable AppContainer/LPAC + Job Object worker boundary. Evidence: `PHASE2_THIRD_SLICE_EVIDENCE.md`.

Current next work: Phase 3 project discovery and indexing. Use `PHASE3_START_HERE.md` as the active implementation authority. Phase 2 contracts remain the production foundation and should not be reopened without contradictory evidence.

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
