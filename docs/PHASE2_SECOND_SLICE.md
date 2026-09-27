# Phase 2 second slice

Status: Complete — accepted on 2026-09-25. Evidence: `PHASE2_SECOND_SLICE_EVIDENCE.md`.

Goal: add the minimum authority, transaction, usage, egress, credential-handle, and actor/delegator foundations required before promoting the adapter broker into production Core.

The slice must prove together:
- command permission and effect-class enforcement before business logic
- trusted transport identity overrides spoofed request-context actor/client/delegator values
- optional project scope fails closed for cross-project reads and writes
- idempotent replay does not duplicate state-changing transaction records
- durable transaction history preserves actor/client/delegator attribution
- usage metrics count commands/replays and leave model/remote costs at zero when unused
- data classification propagates to the strongest source class
- local-only egress denies remote project-data transfer by default and records a durable decision ledger
- credential values never enter command/model-visible context; only scoped credential-handle metadata is persisted
- credential revocation is durable
- schema migration preserves schema-1 and schema-2 stores while moving operational state to schema 3
- graceful/hard restart preserves second-slice durable state
Acceptance:
1. authority checks run after deterministic schema validation and before business logic;
2. command registry permission/effect metadata is enforced by Core;
3. transport establishes trusted local actor/client identity and request payloads cannot self-elevate;
4. transaction, usage, credential metadata, and egress ledger state are durable and versioned;
5. replay of one idempotency key cannot duplicate the corresponding state-changing transaction;
6. remote egress remains default-deny under local-only policy;
7. credential payload values are structurally excluded from egress and command data;
8. schema 1→3 and schema 2→3 migrations preserve prior durable records;
9. live daemon hard-restart verification passes through the canonical CLI transport;
10. resource use remains within the selected Phase 1/first-slice low-footprint class.

No UEFN/Fortnite/tool-specific behavior belongs in this slice.
