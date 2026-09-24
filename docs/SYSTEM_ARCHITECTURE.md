# System Architecture

## Core invariant

Business logic lives once in RELAY Core.

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

### relayd

Persistent local service.

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

Generated or derived surfaces can include CLI help, dashboard forms, skill references, API schemas, and MCP contracts.

This avoids documentation/schema drift.

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

Exact database and object-storage technologies remain open implementation decisions.

## Portability

No adapter may assume one fixed machine path. Paths, ports, detected versions, and integration endpoints belong to installation/project configuration and discovery.
