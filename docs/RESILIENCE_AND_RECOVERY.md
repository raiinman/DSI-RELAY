# Resilience and Recovery

## Purpose

Define how RELAY survives crashes, restarts, storage faults, partial external actions, provider outages, interrupted upgrades, corrupted derived state, and other failures without pretending uncertain state is safe or complete.

## Core invariants

- A crash is an expected operating condition, not an exceptional design case.
- Durable workflow state and external side effects are separate problems.
- A timeout or missing acknowledgement may leave an external effect in an unknown state.
- Recovery must prefer reconciliation over blind retry when an operation may have produced a side effect.
- "Rollback" is used only when the system can actually restore prior state; compensation and manual repair are named separately.
- Project/engine state remains authoritative over rebuildable RELAY indexes.
- Backups are not considered usable until restore/integrity tests succeed.
- Recovery can run in a degraded/reconciling mode instead of falsely reporting healthy state.
- Recovery objectives differ by data class; not every cache deserves the same protection as transactions or user configuration.
- Update and schema migration recovery is designed before automatic migration ships.

## Recovery state model

RELAY should be able to represent states such as:

- Healthy
- Degraded
- Reconciling
- Blocked
- Recovery required
- Manual intervention required

A restarted RELAY instance should not immediately show "healthy" merely because the process is running.

## Operation state machine

Side-effecting operations need durable phase information.

Conceptual states:

~~~
PLANNED
 -> STARTED
 -> EFFECT_REQUESTED
 -> EFFECT_CONFIRMED
 -> VERIFIED
~~~

Failure may produce:

~~~
EFFECT_REQUESTED
 -> UNKNOWN_OUTCOME
 -> RECONCILING
 -> VERIFIED
       or
 -> COMPENSATION_REQUIRED
       or
 -> MANUAL_RECOVERY
~~~

The exact states are an implementation decision, but RELAY must distinguish "failed before action" from "action may have happened."

## Idempotency and retries

Every command should declare retry semantics.

Useful classes:

- Safe read/recompute
- Idempotent write
- Idempotent only with stable key
- At-most-once attempt
- Non-idempotent / reconciliation required

Rules:

- automatic retries are allowed only when the command contract says they are safe
- idempotency keys remain stable across retries of the same logical request
- duplicate queue/job delivery must not imply duplicate side effects
- retry/backoff policies are bounded and observable
- repeated failure enters degraded/blocked state rather than endless loops

## Unknown outcomes

Example:

~~~
RELAY asks external tool to write
external tool applies change
RELAY crashes before persisting success
~~~

On restart, RELAY cannot infer "failed" from missing confirmation.

Recovery must query/reconcile the external system or require manual confirmation when the effect cannot be proven.

Unknown outcome is a first-class result, not an internal exception hidden from the user.

## Rollback versus compensation

Different recovery mechanisms have different guarantees:

### True rollback

The original atomic system can return to the prior state.

### Inverse operation

RELAY can issue a reliable inverse action.

### Compensation

A later action repairs business/project state but does not erase history.

### Restore

A prior snapshot/backup replaces current state.

### Manual recovery

No safe automated method exists.

UI, audit, and APIs must use the correct term.

## Checkpoints

Long-running jobs should durably checkpoint at meaningful boundaries.

Checkpoint design should preserve:

- job ID
- project/revision
- command/version
- completed deterministic stages
- completed external effects/results
- pending/unknown stages
- approvals
- idempotency keys
- adapter/tool versions
- recovery instructions/state

Checkpoints should reduce repeated AI/model calls as well as repeated external effects.

## Queue semantics

Queued work must assume duplicate delivery is possible unless the selected queue/runtime proves a stronger contract relevant to the full side effect.

Requirements:

- stable logical job IDs
- deduplication/idempotency strategy
- leases/heartbeats where workers may die
- bounded retry count/backoff
- poison/dead-letter or blocked-job state
- cancellation/revocation handling
- dependency-aware resume

## Provider and network outages

Local project inspection should remain useful when remote AI or remote gateway services fail.

RELAY should degrade by capability:

~~~
Remote AI unavailable
  -> local deterministic audit still works
  -> local dashboard still works
  -> queued remote-only work waits or fails clearly
~~~

Avoid:

- retry storms
- unbounded queue growth
- repeated user prompts
- silently switching to a different remote processor with different data policy

Circuit breaking/backoff strategy is an implementation decision but outage behavior must be explicit.

## Storage durability

Storage classes need different durability expectations.

Possible classes:

- Critical configuration / identity / policy
- Transaction and approval records
- Job/checkpoint state
- Results/evidence metadata
- Derived indexes/graphs
- Caches

Derived indexes/caches should be rebuildable.

Critical configuration and durable workflow state need recovery/backup requirements appropriate to their role.

## Storage pressure

Disk exhaustion can become a project reliability problem.

RELAY should:

- enforce project/global evidence quotas
- monitor storage growth
- reserve enough headroom for critical transactions/recovery metadata where practical
- prune only according to retention policy
- enter a clear degraded/read-mostly state before uncontrolled disk exhaustion where possible
- never silently delete pinned/critical recovery state

## Backup and restore

RELAY is not automatically the backup system for the user's game/project source.

It should clearly separate:

- backup of RELAY operational state
- backup/export of RELAY project configuration
- user's source-control/project backup
- engine/platform cloud backup where applicable

For RELAY-owned durable state:

- backups need integrity verification
- restore procedures need tests
- backups should have defined retention and encryption policy
- recovery objectives should be documented
- successful backup creation is not equivalent to proven restore capability

## Recovery objectives

Use RPO/RTO concepts where they add value.

Examples:

- project index: RPO effectively rebuildable from source, RTO may be minutes
- cached context: disposable/rebuildable
- transaction/approval history: low tolerated loss
- project configuration/policy: low tolerated loss
- raw evidence: retention-class dependent

Do not force one global RPO/RTO onto all RELAY data.

## Database and storage integrity

On startup or after an unclean shutdown, RELAY should be able to:

- detect unclean/incomplete prior work
- run storage integrity checks appropriate to the chosen database
- detect schema/version mismatch
- restore/rebuild derived data
- reconcile external project/tool state
- withhold "verified current" status until required checks complete

Phase 1 Spike 8 validates the selected Rust + bundled-SQLite path against these rules for schema version 1. Rust and the Node reference both survived a hard process kill with project/result/checkpoint state intact, passed `PRAGMA quick_check` after restart, and opened each other's schema-1 databases without rewriting stored hashes or producer metadata. A malformed database and an intentionally future schema version 999 both caused `Degraded` startup with storage writes blocked; neither store was silently replaced or downgraded. Disk-full injection, backup/restore, concurrent-reader/WAL pressure, maintenance interruption, and interrupted future migrations remain open hardening gates.

## Migrations and upgrades

Schema/data migration is itself a side-effecting workflow.

Before destructive/incompatible migration:

- preflight compatibility
- check available disk/resources
- record source schema/version
- create/verify recovery point when required
- mark migration state durably
- make steps restartable or clearly non-restartable
- define rollback/forward-recovery behavior
- retain old executable/data compatibility long enough to recover where practical

Downgrading application binaries does not imply migrated data is backward compatible.

## Update recovery

Software update and data migration are separate.

An application package rollback may restore old binaries while leaving newer-format data behind.

Update testing must cover:

- crash during install
- crash after binaries updated but before migration
- crash during migration
- migration complete but application health check fails
- downgrade with newer data
- adapter/plugin version mismatch after rollback

## Recovery playbooks

Public releases should include machine-readable and human-readable recovery playbooks for at least:

- corrupted RELAY index
- damaged primary RELAY database
- failed migration
- failed adapter update
- stuck/duplicate job
- unknown external side effect
- remote provider outage
- local host crash/reboot
- disk-full condition

The dashboard should guide users through the relevant playbook without requiring raw database manipulation.

## Fault injection

Recovery should be tested deliberately.

Phase 1+ should inject:

- process kill between durable checkpoints
- machine reboot at write boundaries
- network loss before/after external effect
- duplicate job delivery
- delayed/out-of-order responses
- disk-full / write failure
- corrupted cache/index
- damaged database copy
- adapter crash
- provider outage
- update/migration interruption

The goal is not merely that RELAY restarts; it must recover to a known and explainable state.

## Recovery verification

Recovery ends only after integrity/current-state verification.

Examples:

- database integrity check passes
- expected project state reconciles
- external integrations revalidated
- pending uncertain effects resolved or surfaced
- derived indexes rebuilt/current
- normal operations explicitly resumed

"Process is running" is not a recovery success criterion.

## Open questions

- durable workflow implementation strategy
- exact job/checkpoint schema
- queue implementation
- database technology and integrity tooling
- backup/export format
- default RPO/RTO targets
- snapshot strategy
- disk-reserve policy
- migration framework
- adapter effect-reconciliation interface
- offline/cloud outage queue policy
- disaster-recovery UX


## Observability during recovery

Recovery state depends on the health of the evidence used to verify recovery.

After crash/restart:

- collection adapters must re-establish coverage
- stale pre-crash telemetry must be separated from fresh post-restart evidence
- dropped/unknown event windows should be recorded
- recovery cannot rely only on the same failed monitor that missed the original problem
- normal health status waits for the required observability path to become trustworthy again


## Version skew during recovery

Recovery can involve components or data from different versions.

Requirements:

- recovery records the versions of host, adapters, companions, and storage schema involved
- recovery tools refuse to interpret unsupported future schema as older compatible data
- restored old state is migrated through supported paths before normal operation
- a rollback of binaries does not bypass version-skew/data-compatibility checks
- diagnostic/read-only recovery may be allowed when write compatibility is unavailable

Compatibility and recovery policies must agree; recovery is not a back door around version rules.


## Resource exhaustion recovery

Resource pressure is a recoverable/degraded condition.

RELAY should define behavior for:

- memory pressure
- GPU/VRAM pressure
- disk-full/low-space
- excessive queue backlog
- CPU saturation
- local model OOM/failure

Recovery should prefer pausing/degrading optional work before risking project writes or corrupting durable state.

After pressure clears, deferred jobs revalidate project state before continuing when required.
