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


## Spike 11 strong-isolation fixture

Spike 11 reuses only synthetic workers and adds an adversarial AppContainer/process-sandbox probe. It verifies:

- one broker-owned mailbox directory granted read/write
- one worker executable directory granted read-only
- sibling/ungranted filesystem access denied
- outbound network denied by default
- explicit network grant requires RELAY's capability allowlist plus the measured Windows egress policy
- a minimized explicit environment excludes synthetic parent secrets and `USERPROFILE`
- no inherited handles
- child-process creation remains blocked by the outer one-process Job Object
- unsupported sandbox-spec versions and unsupported RELAY capability names fail before worker launch

The measured `Experimental_CreateProcessInSandbox` backend is a Phase 1 fixture implementation, not the final public adapter sandbox dependency. Spike 12 must reproduce the same adversarial guarantees through a stable/release Windows backend and fallback matrix.
