# Phase 2 Third Slice Evidence

Status: PASS — adapter broker/manifest/isolation vertical accepted on 2026-09-25.

## Scope

This slice promotes the generic Phase 1 adapter boundary into production Rust only:

- `relay-adapter` — manifest validation, lifecycle broker, qualified Windows sandbox backend, Job Object limits, worker contract validation
- `relay-contracts` — existing shared command registry used for adapter command/result/error validation
- synthetic worker and resource-probe binaries — test/evidence only

No UEFN, Fortnite, Blender, Krita, editor-specific, AI-provider-specific, or real external-tool behavior is included.

## Acceptance result

1. Versioned manifest/integrity/provenance boundary: PASS.
2. Permission requests fail closed across network, subprocess, project read/write, credential handle, and external-app classes: PASS.
3. Shared registry command/version/capability validation: PASS.
4. Stable AppContainer/LPAC + Job Object isolation on the measured Windows build: PASS.
5. Filesystem/network/environment/child-process/memory denial semantics: PASS.
6. Response identity/protocol/capability/result/error validation: PASS.
7. Crash/hang/backoff/quarantine/mailbox cleanup: PASS.
8. Post-install artifact revalidation and uninstall lifecycle: PASS.
9. Same-package concurrent invocation serialization and exact descriptor restoration: PASS.
10. Inactive and active resource checks remain in the selected Phase 1 class: PASS.
## Verification

Final exact-tree verification passed:

- `cargo test --workspace`
- `cargo check --workspace --all-targets`
- `cargo build --workspace --release`

Non-doc production tests: 54 total.

Adapter-specific verification:

- 3 backend-selection unit tests
- 8 live/synthetic adapter integration tests
- synthetic binaries/examples compile under the workspace

No new third-party dependency was added. The Cargo lock only gains the local `relay-adapter` workspace package using already-selected `relay-contracts`, `serde`, `serde_json`, `sha2`, and `windows-sys`.

The production workspace explicitly excludes `spikes/phase1/rust`, preserving historical Phase 1 benchmark reproducibility after the production workspace was introduced.
## Security/isolation behavior

The live strong-sandbox test verifies the worker runs as an AppContainer with zero direct capabilities.

The worker can write its ephemeral broker mailbox but cannot:

- read the blocked secret fixture
- write outside the mailbox
- write into its read/execute-only package directory
- connect directly to a loopback TCP listener
- see the synthetic parent secret
- see `USERPROFILE`
- create a child process
- reserve 128 MiB under the 32 MiB Job Object process-memory limit

Windows query-back confirms:

- active-process limit: 1
- process-memory limit: 33,554,432 bytes
- kill-on-close: enabled

The stable release selector enables untrusted launch only on Windows build 26200, the build physically qualified by the Phase 1 adversarial matrix. Simulated unmeasured builds and missing stable APIs fail closed.
## Manifest and worker-contract behavior

Install validates:

- manifest format and identity/version/publisher/trust/update metadata
- target tool/version
- artifact SHA-256
- worker-component SHA-256
- adapter protocol range
- command/version bindings against the shared adapter-exposed registry surface
- requested permission classes against broker policy

Invocation re-verifies the installed worker artifact before launch.

Worker responses must match the launched adapter ID/version, negotiated protocol, process ID, and exact manifest capability set. Successful result data and failed error envelopes are validated against the shared registry. Bad result schemas and undeclared error codes fail closed.

Crash, hang, invalid JSON, identity/version/protocol/capability mismatch, and contract failures feed bounded runtime failure state. Immediate restart backs off; repeated failures quarantine the adapter. Mailboxes are removed after success and failure paths.
## Temporary security mutation and concurrency

The stable sandbox temporarily grants:

- read/write to exactly one ephemeral mailbox
- read/execute to the dedicated adapter package tree
- temporary low-integrity labels required by the selected LPAC boundary

The package security descriptor is verified to restore exactly after invocation.

Production adds one constraint discovered during promotion: each installed worker must live in a dedicated adapter package directory. Pointing the recursive read/execute grant at a shared build tree caused pathological launch cost and would broaden temporary ACL mutation unnecessarily.

Production also adds a per-package invocation lock. Four simultaneous calls sharing one package were tested: all succeeded, the mailbox root was clean afterward, and the package security descriptor restored exactly. This prevents temporary ACL/label restore races without globally serializing unrelated adapter packages.
## Resource evidence

Final release probe, measured while the workstation had active foreground workload:

Zero installed adapters:
- RSS: 5,308,416 bytes
- private memory: 786,432 bytes
- threads: 4
- handles: 66
- five-second sampled CPU: 0 ms
- resident adapter workers: 0

100 validated installed adapters:
- install/validation time: 102.944 ms total
- RSS: 5,955,584 bytes
- private memory: 1,482,752 bytes
- threads: 4
- handles: 66
- five-second sampled CPU: 0 ms
- resident adapter workers: 0

Delta for 100 inactive adapters:
- RSS: +647,168 bytes
- private memory: +696,320 bytes

Thirty full on-demand invocations using a dedicated package directory:
- p50: 113.6006 ms
- p95: 140.7508 ms
- p99: 156.6729 ms
- mean: 116.7214 ms
- min: 104.7318 ms
- max: 156.6729 ms
For a same-load comparison, the historical Phase 1 Spike 12 stable sandbox probe was rebuilt without modifying its committed artifact and measured:

- p50 total: 131.5269 ms
- p95: 171.9732 ms
- p99/max: 183.127 ms
- mean: 134.0289 ms

The promoted production path was therefore about 13.6% faster at p50 under the same active foreground load. The original committed Spike 12 benchmark was faster under a different workstation load and remains the historical reference; this current-load comparison avoids misclassifying foreground contention as a production regression.

The synthetic fixture executable measured 332,800 bytes. The resource-probe example is evidence tooling only and is not a shipping component.

## Promotion fixes discovered

Promotion caught two concrete issues before acceptance:

1. The first production environment block was too small and every AppContainer launch failed with Win32 error 203. Restoring the exact proven minimal environment set fixed launch while still excluding `USERPROFILE` and parent RELAY secrets.
2. The sandbox grants apply recursively to the worker parent directory. The production contract now requires a dedicated adapter package directory; benchmark code was corrected accordingly.

These are evidence that Phase 2 promotion is not a blind source copy.

## Current consequence

The third Phase 2 production slice is accepted.

The adapter lifecycle/isolation row can move from Phase 1 evidence to production. The next work is a Phase 2 closure review against the published Core-and-command-system exit criteria before advancing to Phase 3 project discovery/indexing.
