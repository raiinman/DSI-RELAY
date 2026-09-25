# Synthetic Adapter Fixture

Spike 10 uses a deterministic synthetic worker only. It does not implement any real editor/game/tool behavior.

The broker manifest model includes:

- manifest format version
- adapter ID/version/display name
- publisher/provenance/review metadata
- RELAY adapter-protocol min/max
- command bindings to the trusted RELAY command registry
- requested project/network/credential/subprocess/external-app permissions
- target tool/version requirements
- executable/component version and SHA-256 integrity
- dependency/component metadata
- update channel/source

The worker artifact SHA-256 is computed from the actual generated executable at test/benchmark time and inserted into the in-memory manifest. No generated binary or machine-specific digest is committed here.

Synthetic worker modes cover normal response, crash, hang, invalid JSON, hostile stderr text, subprocess probing, and memory-allocation probing.
