# Purpose

Own production generic adapter manifests, broker lifecycle, and qualified Windows worker isolation.

# Ownership

- Governs `crates/relay-adapter/`.
- Depends on shared command contracts but does not own RELAY Core business logic.
- Real tool-specific adapters remain later integration crates/plugins.

# Local Contracts

- Adapter manifests declare identity/version, publisher/provenance, artifact/component digests, protocol range, registry-bound command capabilities, requested permissions, target requirements, dependencies, trust/review metadata, and update source/channel.
- Manifest requests never grant authority by themselves; broker policy must explicitly allow requested filesystem/project/network/credential/subprocess/external-app scopes.
- Worker executables are SHA-256 verified before launch and a worker component digest must match the launched artifact.
- Untrusted workers are on-demand and out of process.
- Qualified Windows builds use the documented stable AppContainer/LPAC backend selected by D-157; unsupported/unmeasured builds fail closed.
- Direct worker network access stays denied; remote egress must be brokered through a higher-authority RELAY policy path.
- The sandbox grants exactly one direct read/write path: an ephemeral broker mailbox. Worker code directories may be read/execute only.
- Each installed worker must live in a dedicated adapter package directory. The stable sandbox read/execute grant walks the worker's parent package tree; never point it at a shared build, repository, user-profile, or system directory.
- Invocations sharing one adapter package tree are serialized through a package lock so temporary ACL/label grants cannot race. Different package trees may run independently.
- Job Objects enforce one active process, kill-on-close, and a process-memory limit in addition to AppContainer isolation.
- Worker identity/protocol/capabilities and structured result/error schemas are validated against the shared command registry.
- The adapter-only dependency parser operation accepts at most 1 MiB of explicit UTF-8 source content in the ephemeral sandbox mailbox. Broker validation binds observations to the requested source path/hash and rejects unsafe, duplicate, self-referential, or excessive targets before a caller may submit them to Core. A parser result never grants Core write authority.
- Crash/hang/invalid-response failures feed bounded backoff/quarantine state.
- Adapter stdout/stderr and tool-provided text remain untrusted data.
- The synthetic fixture binary is test evidence only and must never be included in public packaging.

# Work Guidance

- Promote only the stable Phase 1 sandbox path; do not ship the experimental processmodel backend.
- Keep Windows ABI/ACL code isolated in the sandbox module.
- Keep Core independent of this crate; daemon/integration layers may compose adapter and Core services later.
- Keep synthetic fixtures generic and public-safe.

# Verification

- Run workspace tests and release build.
- Run live synthetic sandbox integration tests on allowlisted Windows builds.
- Test bad digest, over-permission, incompatible command/protocol, crash, hang, quarantine, direct-network denial, blocked-path denial, child-process denial, temporary ACL restoration, concurrent same-package invocation serialization, and fail-closed unmeasured-build selection.
- Measure inactive installed-adapter overhead and one on-demand invocation against Phase 1 order-of-magnitude evidence using a dedicated adapter package directory.
- Synthetic parser fixture verifies strong sandbox launch, source identity, project-relative target bounds, and malformed observation rejection.

# Child DOX Index

- No child DOX files currently exist.
