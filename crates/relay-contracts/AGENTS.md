# Purpose

Own the versioned machine contracts shared by RELAY Core and every client surface.

# Ownership

- Governs `crates/relay-contracts/`.
- Contains no storage, IPC, UI, adapter, or product-specific business logic.

# Local Contracts

- Own versioned command request/result/error envelopes and protocol constants.
- Own the production command registry and deterministic bounded JSON Schema validation profile.
- Stable command/capability IDs are never silently reused.
- Additive optional fields remain compatible; incompatible requested command versions fail explicitly.
- Discovery metadata must remain compact enough for CLI/dashboard/AI clients.
- No secret values, project paths, or runtime state are embedded in contract metadata.

# Verification

- Registry self-validation.
- malformed arguments/results fail deterministically.
- compatibility tests for optional additions, required-field changes, and version rejection.
- compact discovery tests.

# Child DOX Index

- No child DOX files currently exist.
