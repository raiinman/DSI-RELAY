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

### relayd

Persistent local service. It maintains project indexes, watches integrations, receives queued work, stores results, and performs background health/recovery work.

### relay CLI

Canonical local/headless interface for humans, scripts, CI, and agents with shell access.

### Dashboard

Human control room over the same command system. It shows projects, health, jobs, approvals, tests, assets, history, diagnostics, clients, usage, and advanced details.

### Skills

Compact agent instructions and shell wrappers are the initial local-AI strategy for using RELAY without loading a large tool catalog into context. This is a benchmarked preference, not an assumption: thin/dynamic MCP discovery remains a candidate client surface.

### Remote gateway

Optional path for cloud-only clients that cannot execute commands on the user's machine. It should remain thin and forward structured commands to the local service.

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
- local daemon
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

## Open implementation decisions

These are intentionally not fixed yet:

- primary implementation language/runtime
- local database technology
- dashboard framework
- remote-gateway hosting design
- authentication implementation
- packaging/update technology
- public license
- exact adapter API/ABI
- exact compatibility matrix for UEFN versions

Research and prototypes should resolve these before code locks them in.
