# Versioning and Compatibility

## Purpose

Define how RELAY evolves after users, scripts, AI skills, adapters, dashboards, stored projects, and remote clients depend on its contracts.

The goal is not "never break anything." The goal is predictable change, explicit compatibility, migration paths, bounded support windows, and enough version/capability metadata that mixed versions fail safely instead of silently misbehaving.

## Core invariants

- Semantic version numbers communicate policy; they do not prove compatibility.
- Breaking change analysis includes behavior, error contracts, permissions, rate/latency expectations, schemas, side effects, and persisted data—not only renamed fields.
- Machine-readable contracts are more stable than human-readable presentation.
- Stored results/configuration/evidence retain schema/version metadata.
- Clients and services are assumed to update at different times.
- Version skew is designed and tested; lockstep upgrade is not assumed.
- Capability negotiation is preferred over guessing behavior from a version string alone.
- Removed schema identifiers are not silently reused.
- Deprecation is a lifecycle with notice, migration path, usage evidence where available, and a removal decision.
- Compatibility shims and feature flags have owners and removal criteria.
- Security/correctness can justify an emergency breaking change, but the break must remain explicit and recoverable.
- RELAY does not promise indefinite compatibility for every experimental surface.
- Public/stable and experimental contracts have different support promises.

## Contract inventory

RELAY will have multiple compatibility surfaces:

- CLI commands, flags, exit codes, structured output
- local IPC protocol
- remote gateway/API
- MCP surface
- command registry/schema
- adapter SDK/protocol
- adapter manifests
- dashboard-to-core protocol
- project configuration
- database/storage schema
- result/evidence envelopes
- transaction/job/checkpoint state
- AI skills/wrappers
- diagnostic bundle format
- exported/imported project metadata
- runtime instrumentation protocol
- companion-component protocols
- extension/package metadata

Each durable contract needs an owner, stability class, and version/evolution policy.

## Stability classes

Conceptual stability classes:

### Internal

No public compatibility guarantee. May change freely with the owning release as long as migrations/tests remain correct.

### Experimental

May change or disappear. Version/capability checks are mandatory and users/adapter authors must be warned not to treat the contract as stable.

### Preview

Intended for public use but still evolving. Breaking changes require notice/migration guidance, but support windows can be shorter than stable surfaces.

### Stable

Breaking changes require explicit versioning or a defined deprecation/removal process.

The exact names may change, but RELAY should not promise stable compatibility for everything it exposes.

## Version versus capability

Versions answer:

"What release/protocol generation is this?"

Capabilities answer:

"What can this specific peer actually do?"

RELAY should use both.

Example:

~~~
RELAY host: 1.4
protocol: 3
capabilities:
  result.query@2
  uefn.spawn.inspect@1
  adapter.sandbox.appcontainer=false
~~~

A client should not infer a capability merely because a release number is high enough when capability negotiation is available.

## Version skew

Mixed versions are normal:

- CLI updated before local host
- dashboard cached/old while host updated
- remote gateway newer than local host
- adapter older than RELAY Core
- companion plugin newer/older than adapter worker
- AI skill generated for an earlier command contract
- restored project data from an older release

Every interface should define the supported skew rather than assume exact equality.

When skew is unsupported:

- fail closed for writes
- keep safe read/diagnostic paths when possible
- explain the incompatibility plainly
- provide the minimum supported upgrade/downgrade path

## API and command evolution

A breaking change can include:

- removing/renaming commands, fields, flags, or parameters
- changing types or requiredness
- changing behavior/side effects
- changing permission requirements
- changing top-level error/exit semantics
- changing idempotency/retry behavior
- changing ordering/pagination guarantees
- materially changing latency/rate/concurrency contracts if clients depend on them
- changing default privacy/egress behavior

Public stable contracts should evolve additively where practical.

## CLI compatibility

Human CLI text is presentation, not the machine contract.

Rules:

- scripts/agents should use structured output
- structured output declares schema/contract version
- stable exit-code categories are versioned contracts
- field meaning is not silently changed
- human text may improve without being parsed by machines
- deprecated commands/flags produce actionable migration guidance
- shell completion/help is generated from current command metadata

A command can remain available as a compatibility alias without exposing it to new AI contexts by default.

## Structured schema evolution

The serialization format affects evolution rules.

If RELAY adopts Protocol Buffers or another IDL:

- follow the chosen format's actual backward/forward compatibility rules
- never reuse retired field/tag identifiers when the format forbids/sensitively handles reuse
- reserve removed identifiers where supported
- keep unknown-field behavior in mind
- test binary and JSON/text representations separately if both are used

A schema change that is safe in one representation may be unsafe in another.

## Result and evidence longevity

Stored results may outlive the code that produced them.

Each durable result should retain:

- result schema version
- producing RELAY version
- command/rule/adapter version
- relevant project/session revision
- raw evidence reference/version where retained

New RELAY versions may render older results, but must not silently reinterpret old fields using new semantics.

If an old result cannot be faithfully interpreted:

~~~
Historical result
Produced by RELAY 1.3
Some fields cannot be rendered by this version.
Raw record remains available.
~~~

is better than fabricated compatibility.

## Persistent data schema

Persistent data needs explicit migration chains.

Requirements:

- schema version stored durably
- migrations are ordered/versioned
- compatibility/preflight checks before migration
- recovery point when required
- migrated state validates against target schema
- old binaries refuse unsupported newer data safely
- migration provenance/history retained where useful

See RESILIENCE_AND_RECOVERY.md for rollback/restore rules.

## Adapter protocol evolution

Adapter compatibility is declared, not guessed.

Adapter manifests should declare:

- RELAY protocol/API range
- command/schema versions
- companion-component versions
- external-tool version/capability requirements

The adapter broker negotiates capabilities and quarantines unsupported combinations.

The protocol should prefer additive changes and optional capabilities over forcing every adapter to release in lockstep.

## AI skill evolution

Skills are versioned clients.

A skill should identify:

- compatible RELAY contract range
- command/schema version generated from
- optional domain capabilities

RELAY should be able to detect stale skills and regenerate/update them.

Do not keep loading obsolete command descriptions merely to preserve a legacy skill; migration should update the skill or route through a bounded compatibility alias.

## Deprecation lifecycle

A stable public element should move through a lifecycle such as:

~~~
Active
 -> Deprecated
 -> Migration available
 -> Removal eligible
 -> Removed
~~~

Deprecation metadata should include:

- deprecated since/version
- replacement/migration path
- planned earliest removal date or support rule
- compatibility impact
- usage signal where available and privacy-safe
- owner

Removal should require more than elapsed time when high-impact usage is known.

## Deprecation communication

For remote APIs, standards such as HTTP Deprecation/Sunset can inform clients.

For RELAY's local surfaces use equivalent structured metadata in:

- CLI warnings
- dashboard health
- adapter compatibility
- generated docs/skills
- API/MCP responses where appropriate

Warnings should be deduplicated and avoid training users to ignore them.

## Usage-aware retirement

Where privacy-safe telemetry or local inspection can show whether a deprecated contract is still used, use that evidence in removal decisions.

Do not require cloud telemetry for this; local compatibility scans can detect:

- old config schema
- deprecated CLI wrapper references
- stale generated skills
- installed adapters using old contracts
- project manifests with legacy fields

Unknown usage remains unknown rather than assumed zero.

## Compatibility shims

A shim has a cost.

Every shim should have:

- reason
- owner
- scope
- supported versions
- tests
- observability/usage signal
- removal condition
- deadline/review date where practical

Compatibility code without retirement criteria becomes permanent architecture.

## Feature flags

Feature flags are useful for staged rollout and migration but can become long-lived state.

Requirements:

- owner
- introduction version
- purpose
- default
- rollout status
- compatibility implications
- removal condition
- review/expiry target

Flags affecting schemas/protocol behavior need explicit mixed-version tests.

## Rolling upgrades

Upgrade order matters when multiple RELAY components coexist.

Document supported order, for example:

1. ensure current installation is healthy
2. upgrade protocol-compatible host/gateway components
3. migrate storage if required
4. upgrade dashboard/CLI
5. upgrade adapters/companions
6. retire old compatibility paths after skew window

The exact order depends on chosen architecture, but mixed-version states must be deliberately tested.

## Security breaking changes

Sometimes maintaining compatibility is less important than fixing a serious vulnerability or unsafe behavior.

Emergency break policy should specify:

- severity threshold
- affected versions
- mitigation
- migration/upgrade path
- compatibility impact
- rollback limitations
- communication

Do not disguise a security break as a non-breaking patch when client behavior changes materially.

## Support windows

Public release needs a bounded support policy.

Decide before GA:

- number/duration of supported minor/major lines
- security-fix policy
- adapter SDK support window
- project/schema migration support
- CLI/API deprecation window
- old skill compatibility
- whether LTS releases exist

Avoid promises that cannot be staffed.

## Compatibility test matrix

CI/release tests should include:

- previous supported CLI against current host
- current CLI against previous supported host where permitted
- previous/current dashboard
- previous/current gateway
- previous/current adapter protocol
- stored data from every supported migration source
- current code reading historical results
- rolling upgrade sequence
- rollback/downgrade rejection
- unknown/new fields and enums
- removed/reserved schema identifiers
- deprecated commands/flags
- stale skills
- unsupported skew failure behavior

## Open questions

- contract/stability taxonomy names
- public API version format
- local IPC serialization
- schema format/IDL
- support window lengths
- preview/experimental policy
- compatibility alias duration
- telemetry/local usage signals for deprecation
- LTS strategy
- adapter SDK release cadence
- skill auto-regeneration/update mechanism
- historical-result rendering policy
