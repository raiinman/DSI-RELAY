# Phase 1 Command Contracts

`commands.registry.json` is the selected Phase 1 semantic source for built-in RELAY command metadata.

It currently owns:

- command ID + contract version
- concise purpose
- argument/result schemas
- declared command errors
- effect, permission, and idempotency classes
- allowed discovery/presentation surfaces
- reserved/deprecated command IDs

The registry declares JSON Schema 2020-12, while the Rust prototype deliberately supports only the bounded keyword profile validated in `../rust/src/registry.rs`. Unsupported keywords fail startup validation.

Discovery is progressive:

1. `registry.list` returns compact metadata.
2. `registry.describe` returns one full command contract.

The 13-command catalog is not a frozen public API. D-154 selects the mechanism and compatibility behavior, not every current command forever.
