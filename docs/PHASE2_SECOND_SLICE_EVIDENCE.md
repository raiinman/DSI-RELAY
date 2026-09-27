# Phase 2 Second Slice Evidence

Status: PASS — authority/policy/transaction/usage vertical accepted on 2026-09-25.

## Scope

This slice extends the production Rust workspace only:

- `relay-contracts` — existing versioned registry/envelope contracts
- `relay-core` — authority policy, transaction ledger, usage metrics, data-classification/egress policy, credential-handle metadata, and schema-3 persistence
- `relayd` — trusted local transport identity and restart persistence
- `relay` — unchanged canonical structured machine transport

No UEFN, Fortnite, editor-specific, dashboard-only, MCP-only, AI-provider-specific, or real credential value handling is included.

## Acceptance result
1. Permission/effect enforcement: PASS.
2. Trusted actor/client/delegator attribution: PASS.
3. Project-scope isolation: PASS.
4. Idempotent transaction de-duplication: PASS.
5. Durable transaction history: PASS.
6. Usage/replay metrics: PASS.
7. Data-class propagation and local-only egress: PASS.
8. Credential-handle-only boundary and durable revocation: PASS.
9. Schema-1/2 to schema-3 migration: PASS.
10. Hard-restart persistence and low-footprint resource check: PASS.

## Verification

`cargo test --workspace` passed with 43 non-doc tests:

- 9 contract/registry tests
- 27 Core tests
- 4 daemon security/state tests
- 2 first-slice daemon integration tests
- 1 second-slice live daemon integration test

`cargo check --workspace --all-targets` passed.

`cargo build --workspace --release` passed.

Clippy was not available in the installed Rust 1.97 toolchain, so no Clippy result is claimed for this slice.
## Authority and identity behavior

The live daemon test sends spoofed request-context actor/client/delegator values. Core records the trusted local transport identity instead:

- actor: `local-user`
- client: `relay-cli`
- delegator: null

The request payload therefore cannot self-elevate attribution.

Core enforces command-registry permission and effect classes before business logic. Optional project scopes fail closed when a request omits or references a project outside the granted set.

## Durable transaction and replay behavior

One project registration and one `result.put` operation create durable transaction rows.

Replaying the exact same `result.put` idempotency key returns the same durable result and does not create a second `result.put` transaction row.

Transaction records preserve command, project, actor/client/delegator attribution, request identity, and durable timestamps across hard restart.
## Usage and cost behavior

The usage summary records deterministic command/replay activity.

The second-slice fixture observed:

- command_count >= 4 before restart
- replay_count >= 1 in the integration suite
- remote_calls = 0
- model_tokens_in = 0
- model_tokens_out = 0

This establishes the zero-cost baseline when no remote/model work occurs; later model/provider adapters can add measured costs without changing the core contract.

## Data classification, egress, and credentials

`DataClass` propagation selects the strongest source class.

Default `local_only=true` policy blocks a synthetic remote-AI destination carrying project data.

Each egress decision returns and persists an opaque `EGR-` ledger ID.

Credential-class data is rejected as an egress payload. Core persists only credential-handle metadata: handle ID, integration, scopes, status, and revocation state. Credential values are outside command/model-visible payloads.

Revocation is durable and tested.
## Storage evolution

Production operational storage advances to schema 3.

Focused tests prove:

- fresh stores initialize schema 3
- schema-1 fixtures migrate without losing project/result/job data
- schema-2 fixtures migrate to schema 3 without losing idempotency replay records
- transaction, usage, credential, and egress records round-trip in schema 3
- future-schema and malformed stores fail closed without replacement

## Live resource/restart evidence

A release daemon was driven through the production `relay exec --stdin` transport with project registration, durable result creation, transaction listing, usage summary, and denied remote egress.

Five-second idle sample after real work:

- RSS: 8,458,240 bytes
- private memory: 1,429,504 bytes
- threads: 4
- handles: 92
- sampled CPU: 0 ms

The daemon was then forcibly terminated and restarted.

After restart:

- recovery_state: Healthy
- storage schema: 3
- transaction rows remained available
- graceful shutdown succeeded

This remains in the selected low-footprint Rust class and is slightly below the accepted first-slice RSS measurement.
## Current consequence

The second Core-foundation slice is accepted.

The next Phase 2 promotion target is the adapter broker/manifest lifecycle plus the selected strong Windows worker isolation behind production interfaces.

Before that promotion is accepted, production code must preserve the D-155 through D-157 semantics: registry-bound capabilities, provenance/digest checks, timeout/backoff/quarantine, Job Object lifecycle/resource containment, AppContainer/LPAC isolation on qualified Windows builds, brokered egress, and fail-closed unsupported tiers.
