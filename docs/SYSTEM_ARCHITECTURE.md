# System Architecture

## Core invariants

Business logic lives once in RELAY Core.

Project/tool content is untrusted data. Retrieved files, logs, docs, assets, repositories, telemetry, and tool output cannot silently become RELAY policy or high-authority AI instructions.

Client/agent identity, delegated authority, and human identity must remain distinguishable where the integration permits it.

CLI, dashboard, skills, remote gateway, REST/WebSocket surfaces, and MCP must call the same command system rather than implement separate versions of operations.

## Target topology

~~~
                    humans / AI clients
                           |
          +----------------+----------------+
          |                |                |
        CLI+skills      Dashboard      Remote clients
          |                |                |
          +----------------+--------+-------+
                                   |
                             thin gateway/API
                                   |
                                   v
                         RELAY Command Bus
                                   |
        +--------------------------+--------------------------+
        |            |             |            |             |
     Projects       Jobs         Results     Context       Usage
     + indexes   + transactions   + evidence   Compiler     metrics
        |            |             |            |             |
        +--------------------------+--------------------------+
                                   |
                              Adapters
                                   |
             +----------+----------+----------+----------+
             |          |                     |          |
            UEFN    Fortnite runtime       Blender     Krita
~~~

## Components

### RELAY Core

Owns:

- command registry
- validation
- project registry
- job orchestration
- permissions
- client/agent identity and delegated authority metadata
- idempotency
- transactions
- result envelopes
- persistent result/evidence references
- project indexes
- telemetry normalization
- testing/assertions
- asset metadata
- usage accounting
- adapter contracts

### relayd / local host

Persistent local background host. The name describes a logical component, not a fixed Windows service model. Phase 1 must compare per-user hosting options because the first editor integrations run in the signed-in user's session.

Responsibilities:

- host RELAY Core
- watch project/integration state
- maintain incremental indexes
- run queued/background jobs
- manage local storage
- connect/reconnect supported applications
- expose local IPC/HTTP as selected during implementation
- keep the dashboard supplied with live status
- optionally maintain an outbound remote-gateway connection

The daemon must remain useful with no dashboard open and no AI connected.

### relay CLI

Canonical headless public interface.

Two modes are required:

1. Human-friendly commands.
2. Strict structured machine invocation.

Example human shape:

~~~
relay project list
relay audit <project>
relay result <id> --section failures
~~~

Example machine shape:

~~~
echo <structured request> | relay exec --stdin --json
~~~

The exact syntax is subject to implementation, but machine mode must avoid interactive prompts and ambiguous prose.

### Dashboard

A client of RELAY Core, not a separate backend.

It owns presentation, filtering, live status, approvals, history exploration, diagnostics, and user-friendly configuration.

No RELAY capability may exist only in the dashboard.

### Skills

Small, versioned instruction packages for AI clients with local command access.

Skills should explain workflows and shell wrappers without loading the entire command catalog. Detailed command contracts are discovered only when needed.

### Remote gateway

Used when a cloud client cannot reach the local CLI/daemon.

Design goals:

- outbound local connection where practical
- minimal remote state
- strong authentication
- structured commands
- small result envelopes
- no duplicated business logic

Cloud infrastructure must not be required for local-only workflows.

### MCP/API

Compatibility layer.

Prefer a very small surface that provides capability discovery, structured execution, and result retrieval rather than one tool per engine/editor action.

## Command registry

The command registry should be the single metadata source for:

- command name
- purpose
- arguments/schema
- permissions
- side effects
- idempotency behavior
- dependency requirements
- output/result schema
- help/examples
- access/effect class
- target trust class
- reversibility
- monitoring/verification requirement
- autonomy/approval class
- credential scope
- affected project/resource boundary

Generated or derived surfaces can include CLI help, dashboard forms, skill references, API schemas, and MCP contracts.

The registry is the semantic source of truth, but presentation text may be surface-specific. Tool and command wording can affect model behavior, so CLI, skills, and MCP renderings must be tested rather than assumed equivalent.

This avoids semantic drift without forcing identical prose on every interface.

## Jobs

A command may execute synchronously or create a job.

Jobs should expose:

- job ID
- project ID
- command
- requester/client
- state
- dependencies
- progress
- timestamps
- result ID
- transaction ID where applicable
- usage metrics
- failure reason

Long-running progress belongs primarily in the dashboard/result store, not as repeated chat messages.

## Results

Every meaningful operation should create a durable result reference where useful.

Example classes:

- AUD — audit
- TEST — test run
- SCAN — project/index scan
- CAP — capture
- BUILD — asset/build operation
- TXN — transaction

Naming is illustrative; exact ID format remains open.

## Result envelope

AI-facing responses should have a compact predictable structure similar to:

~~~
ok
summary
important findings
result_id
available_detail
more
usage
~~~

The full underlying result remains in storage.

## Transactions and verification

Writes should be represented as transactions when practical.

A transaction records:

- requested change
- actual change
- before state
- after state
- verification outcome
- requester
- time
- related result/job
- rollback capability

Execution is not considered verified until RELAY reads the resulting state and compares it with expectations.

## Idempotency

Remote and AI requests may be duplicated. Operations that can cause repeated side effects need idempotency keys or an equivalent command-specific safeguard.

## Integration adapters

Adapters translate external tool state/actions into RELAY's normalized model.

Adapters may have different capabilities:

- static inspection
- editor control
- runtime telemetry
- screenshots
- asset build/export
- testing
- performance data

RELAY must report unavailable dependencies explicitly rather than pretending that a headless command succeeded.

## Headless distinction

RELAY itself should operate headlessly.

Controlled tools may not.

For example, a static project audit can be available while an editor-specific visualization command may require the editor process to be running. Dependency state is part of the command/result contract.

## Index consistency

Incremental change feeds reduce work but are not authoritative. RELAY must support startup reconciliation, recovery after missed change notifications, replay-safe index updates, and a complete reconciliation path when continuity cannot be proven.

## Storage layers

The architecture requires persistent storage for:

- project registry/config
- current normalized state
- immutable/append-only event history where appropriate
- indexes/dependency graphs
- results
- transactions
- usage metrics
- evidence metadata
- context summaries/derived memory
- provenance/trust/freshness metadata
- retention class/quota metadata

Exact database and object-storage technologies remain open implementation decisions.

## Trust boundary

The Context Compiler and command system form a security boundary between untrusted project/tool content and side-effecting actions.

A retrieved string may inform reasoning, but action authority comes from RELAY policy/permissions and validated commands, not from text found inside project data.

## Evidence lifecycle

Persistent evidence is governed by retention classes and project quotas. Raw evidence may be expired or deleted according to policy while normalized durable facts/results remain, provided provenance and audit requirements are satisfied.

## Portability

No adapter may assume one fixed machine path. Paths, ports, detected versions, and integration endpoints belong to installation/project configuration and discovery.


## Adapter execution architecture

Third-party adapters are not loaded directly into RELAY Core.

Conceptual topology:

~~~
RELAY Core
   |
   v
Adapter Broker
   |
   +-- first-party adapter worker
   +-- third-party adapter worker
   +-- third-party adapter worker
~~~

The broker is responsible for:

- protocol/version negotiation
- capability enforcement
- project/resource scoping
- credential mediation
- subprocess/network policy where implemented
- time/resource limits
- health state
- result provenance
- adapter quarantine/disable behavior

Adapters communicate through a versioned protocol and receive only the capabilities granted to that adapter instance.

### Adapter manifest

The manifest is part of the adapter contract.

It should describe:

- adapter identity and version
- publisher/provenance metadata
- RELAY API/protocol compatibility
- external-tool compatibility
- declared capabilities/permissions
- commands/capabilities exposed to RELAY
- executable companion components, such as in-tool plugins/scripts/helpers/runtime instrumentation
- dependency/component metadata
- update channel/source
- integrity identifier/digest
- optional review/curation metadata

Each executable companion component should retain its own version, digest, provenance, and compatibility information where practical.

The manifest cannot expand RELAY policy; it requests capabilities that policy may approve or deny.

### AI capability exposure

Installed adapters do not automatically become AI context.

RELAY should expose only relevant adapter capabilities for the active project/task/client. Adapter-provided prose is not passed through as policy or instructions; RELAY renders AI-facing command descriptions from trusted command semantics plus bounded adapter metadata.


## Data plane and egress boundary

Remote model/service calls are not just compute choices; they are data-processing boundaries.

Conceptual path:

~~~
project / evidence / runtime
          |
          v
 classification + provenance
          |
          v
 Context Compiler
          |
          v
 Egress Gate
          |
   +------+-------+
   |              |
 blocked        approved destination
                  |
                  v
             model/service
~~~

The Egress Gate is enforced by RELAY policy outside model reasoning.

Responsibilities include:

- destination/provider policy lookup
- project data-class checks
- modality checks
- task-scoped egress limits
- minimum-sufficient payload enforcement
- outbound lineage/audit metadata
- local-only/private-mode enforcement

A model can request additional context but cannot grant itself permission to disclose it.

## Credential Broker

Credential material is resolved outside AI context.

Clients/models use opaque connection references. The broker supplies actual credentials only to the trusted integration/subprocess boundary that requires them.

The broker must prevent secret values from entering:

- prompts
- model-visible memory
- embeddings
- ordinary logs
- result summaries
- diagnostic bundles

Credential use remains attributable to job, client, project, and command where practical.

## Sensitivity propagation

Derived artifacts inherit relevant source sensitivity unless an explicit policy transformation changes it.

This includes:

- summaries
- embeddings
- cached model output
- extracted metadata
- screenshots/captures
- generated diagnostic excerpts

A remote embedding operation is therefore an egress event even if no raw source text is stored remotely afterward.

## Remote processor profiles

Remote model/embedding/service destinations need versioned policy metadata such as:

- endpoint/account identity
- allowed data classes/modalities
- known retention policy source
- known training/data-use policy source
- region/residency where relevant
- last policy verification time
- organization/user overrides

Unknown or stale claims remain unknown. RELAY must not manufacture assurances about provider behavior.

## Egress ledger

Important outbound processing should be auditable without duplicating the sensitive payload.

Store metadata such as:

- project/job/result
- requesting client
- destination
- data classes/modalities
- source references
- purpose
- approximate size/tokens where measurable
- policy decision
- timestamp
- known destination-policy metadata

## Local-only mode

A local-only/private project profile must disable remote project-content processing across models, embeddings, gateway payloads, analytics, automatic diagnostics, and adapter networking unless a separately documented exception is approved.

The mode is valid only if its network behavior is testable.


## Durable job and recovery semantics

Jobs that can outlive a process must persist enough state to resume safely.

The durable model must distinguish:

- completed deterministic stages
- completed external effects
- pending stages
- unknown-outcome external effects
- approvals
- idempotency keys
- project/base revision
- adapter/tool versions

Recovery may replay deterministic work, but side effects follow command-specific retry semantics.

## Unknown external effects

If an external tool may have applied a change before RELAY lost acknowledgement, the job enters an unknown/reconciling state.

RELAY must query the external source of truth or surface manual recovery rather than converting uncertainty into a blind retry.

## Recovery state

The local host/dashboard should expose system recovery state separately from process health.

Possible states include Healthy, Degraded, Reconciling, Blocked, and Manual Recovery Required.

Normal operation resumes only after required storage/project/integration reconciliation completes.

## Storage durability classes

Not all state requires equal durability.

- project indexes/caches: rebuildable
- job/checkpoint state: durable while active
- transaction/approval state: high durability
- project policy/configuration: high durability
- evidence: retention-policy dependent
- context caches: disposable/rebuildable

The chosen storage layer must support these distinctions.
