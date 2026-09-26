# RELAY Project Plan

## North star

RELAY should let a creator, developer, or AI inspect and operate a supported development project without forcing the AI to micromanage the editor, consume huge context windows, or depend on an expensive coding agent.

A typical successful interaction should look like:

~~~
one small request
      |
      v
RELAY performs many local deterministic operations
      |
      v
one compact verified result
      |
      v
AI or human decides the next meaningful action
~~~

The full evidence remains available in RELAY without being dumped into chat.

## Product goals

1. Reduce AI usage cost across the entire workflow.
2. Make normal chat-based AI useful, not only heavyweight coding agents.
3. Give AI reliable visibility into project and runtime state.
4. Keep project state and evidence outside the conversation.
5. Make operations reproducible through a canonical headless interface.
6. Make the dashboard understandable to people who do not know MCP or agent infrastructure.
7. Support multiple isolated projects.
8. Keep AI providers and transport standards replaceable.
9. Create a practical first integration for UEFN/Fortnite while preserving a general core.
10. Build toward a public-installable product.
11. Keep RELAY simpler to operate than the development complexity it removes.

## Non-goals

- RELAY is not an AI model.
- RELAY is not a replacement for the game engine or creative application.
- RELAY is not a giant catalog of one MCP tool per editor action.
- RELAY does not use the chat transcript as the project database.
- RELAY does not silently publish, delete, spend money, or alter credentials.
- RELAY does not require one specific project, repository, AI vendor, or paid model.

## Architectural plan

### RELAY Core

Owns command execution, validation, project state, results, transactions, tests, telemetry, usage accounting, and integration contracts.

### relayd / local host

Logical persistent background host for RELAY Core. Phase 1 must compare a normal per-user background process, Windows per-user service options, and any narrowly scoped helper needs rather than assuming a traditional system service. It maintains project indexes, watches integrations, receives queued work, stores results, and performs background health/recovery work.

### relay CLI

Canonical local/headless interface for humans, scripts, CI, and agents with shell access.

### Dashboard

Human control room over the same command system. It shows projects, health, jobs, approvals, tests, assets, history, diagnostics, clients, usage, and advanced details.

### Skills

Compact agent instructions and shell wrappers are the initial local-AI strategy for using RELAY without loading a large tool catalog into context. This is a benchmarked preference, not an assumption: thin/dynamic MCP discovery remains a candidate client surface.

### Remote gateway

Optional path for cloud-only clients that cannot execute commands on the user's machine. It should remain thin and forward structured commands to the local service.

### Data boundary

Remote models, embedding services, gateways, networked adapters, and support uploads are explicit external processing destinations. RELAY policy decides what project data may leave the local environment before the Context Compiler optimizes the payload. Credentials are resolved through opaque handles outside model context.

### MCP/API adapters

Compatibility surfaces only. They must not duplicate business logic or become the primary architecture.

## Data strategy

RELAY keeps three distinct layers:

1. Raw evidence — logs, screenshots, tool output, telemetry, files, measurements.
2. Structured state/history — entities, results, transactions, dependencies, current state, immutable events.
3. Compiled AI context — small task-specific packages generated for a goal and token budget, with provenance, intent, freshness, trust, conflict, and exact-field constraints.

Removing data from an AI context never deletes the underlying evidence.

## Operation strategy

A meaningful write should follow:

~~~
inspect
  -> plan/dry-run when appropriate
  -> execute
  -> read actual state
  -> verify expected vs actual
  -> record transaction/result
  -> expose rollback where technically possible
~~~

Repeated requests should use idempotency keys or equivalent safeguards.

## Multi-project model

Each project owns isolated:

- identity and configuration
- integration state
- indexes
- assets
- tests
- telemetry
- results
- transactions
- permissions
- context/memory
- usage metrics

Core code must not hard-code project-specific names or rules.

## First implementation target

UEFN/Fortnite is the first real integration because it exercises the difficult parts of RELAY:

- editor state
- runtime visibility
- Verse
- devices and entities
- play sessions
- visual debugging
- screenshots
- asset workflows
- performance/memory checks
- project-specific gameplay tests

Blender and Krita are companion asset integrations.

## v0.1 target

The first usable milestone should prove the architecture rather than attempt universal coverage.

Required capabilities:

- project registry and onboarding
- per-user local host foundation
- command registry and structured execution
- CLI
- persistent result store
- compact result envelopes
- project inspection
- incremental indexing foundation
- UEFN connection/status
- spawn discovery and visualization
- structured runtime telemetry
- runtime probes foundation
- gameplay assertion/test harness
- fixed-camera capture foundation
- asset manifest and validation foundation
- relay audit
- transaction/history foundation
- dashboard with status, jobs, findings, approvals, history, integrations, and usage
- compact agent skill
- usage measurements

## Success criteria

v0.1 is successful when a new supported project can be added, inspected, audited, and tested without requiring a model to enumerate the project manually; important operations are available headlessly; the same operation can be invoked from CLI and dashboard; results are durable and referenceable by ID; large outputs stay in RELAY; and usage metrics demonstrate how much work was performed locally versus sent to AI.

## Public-release constraints

From the first implementation:

- no personal filesystem paths
- no embedded credentials
- no dependency on a private repository
- no hidden assumptions about one machine
- migration/versioning strategy for stored data
- diagnostics that redact secrets
- clear separation between local-only and remote capabilities
- project isolation
- install/update path designed for non-experts
- project/tool content treated as untrusted input
- scoped/revocable client/agent authority where supported
- evidence retention/quota/privacy lifecycle
- native-tool-first adapter policy
- benchmarked defaults rather than single-demo optimization
- third-party adapters isolated from RELAY Core by default
- adapter capability manifests, exact-version compatibility, provenance, and update/recovery controls
- no requirement for an open extension marketplace before these controls are proven
- explicit project data classification and destination-specific egress policy
- raw credentials excluded from model-visible context
- sensitive derived artifacts inherit source policy
- local-only/private mode defined by testable network behavior rather than UI wording
- team/workspace identity separated from AI client and agent identity
- project-scoped authorization and delegated-action attribution
- stale-plan/revision checks for important collaborative writes
- offboarding semantics for active and queued work
- durable job recovery with explicit unknown-outcome handling
- tested restore/recovery procedures for RELAY-owned state
- migration/update recovery separated from binary rollback
- graceful degraded operation during remote-provider outage
- a personal golden path that provides useful local value before remote AI or team configuration
- secure/sensible defaults that avoid mandatory policy hardening
- configuration and feature growth governed by explicit complexity budgets
- common diagnostics available through one low-friction health path
- public distribution does not rely on unreviewed platform-term assumptions
- dependency/companion/asset licensing and required notices are tracked before release
- user-facing AI/copyright/privacy claims stay narrower than the evidence supports
- stable machine contracts have explicit version/evolution policies
- mixed-version clients/adapters fail safely rather than assuming lockstep upgrades
- deprecated compatibility surfaces have migration and retirement plans
- historical results/configuration remain interpretable or explicitly marked unsupported

## Phase 1 implementation selections

D-153 selects Rust as the local host/runtime foundation and bundled SQLite through minimal `rusqlite` for operational metadata, compact results, jobs/checkpoints, and migration state. The Node implementation remains a compatibility/reference fixture during Phase 1.

The dashboard command/transport boundary is selected as a thin static/HTTP surface hosted by the same local core process, but the final desktop renderer/wrapper technology is intentionally still open.

## Open implementation decisions

These are intentionally not fixed yet:

- final dashboard desktop renderer/wrapper technology
- remote-gateway hosting design
- authentication implementation
- packaging/update technology
- public license and contributor copyright model
- exact adapter API/ABI
- exact compatibility matrix for UEFN versions

Research and prototypes should resolve these before code locks them in.


## Observability quality constraint

RELAY's value depends on trustworthy evidence, not just more telemetry.

The product must preserve source/freshness/sampling/verification state for important evidence, track health of the observability path, benchmark instrumentation overhead, and distinguish measured facts from inferred causes.

A low-cost system that confidently reasons from stale or incomplete telemetry fails the product goal.


## Performance and resource economics

RELAY's cost goal includes the creator's workstation, not only model/API usage.

Product requirements include:

- foreground creator workload priority
- progressive project readiness before deep indexing completes
- bounded idle CPU/RAM/disk/network cost
- inactive-project quiescence
- local-model routing that considers GPU/VRAM contention
- tail-latency/interactive performance benchmarks
- storage/index maintenance budgets
- hardware-tier performance fixtures
- long-run resource-aging tests

A token-saving feature that materially degrades UEFN/creator responsiveness is a product regression.
