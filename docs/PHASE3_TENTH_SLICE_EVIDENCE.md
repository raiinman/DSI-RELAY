# Phase 3 parser health evidence

Status: PASS for operator-visible synthetic parser failures and recovery, 2026-09-26. Supported-tier and real-adapter proof remain open.

## Contract

`system.status` adds a bounded `host_components` array. The daemon supplies one generic host component for `adapter.dependencies.parse` with state `not_configured`, `healthy`, or `degraded`; a fixed error code; and capped installed, declared, unavailable, quarantined, failure, and success counts. No source path, source content, worker message, package path, or project ID enters this aggregate result. `system.doctor` includes the component as a pass/fail check and marks overall health degraded when parser delivery needs attention. Both use the existing canonical command path and an additive status-result schema field.

Malformed or digest-mismatched installation files grant no parser access while the daemon remains available to report `PARSER_INSTALLATION_INVALID`. A declared project without an installed grant reports `PARSER_NOT_INSTALLED`; an exact-version mismatch reports `PARSER_VERSION_MISMATCH`. Worker, observation, and edge-write failures update an in-memory per-project state. Only fixed error codes and transition events enter the bounded Core JSONL diagnostics; repeated identical retries do not emit repeated transition events. A quarantined adapter reports `ADAPTER_QUARANTINED`. When the indexed source digest changes, the daemon may clear that package's quarantine and retry through the same manifest verification and sandbox. A successful guarded replacement clears the active failure and emits a recovery event. Historical counters reset on daemon restart; source and edge authority remain unchanged.

The parser retry timer now applies only to the same ready index generation and source digest. A successful parse clears its attempt record, and a newly indexed source bypasses an earlier failed-attempt delay. Failed attempts remain throttled. This preserves the one-source-per-poll idle schedule while avoiding an unintended five-second wait after ordinary edits.

## Verification

- Four live daemon tests in `phase3_parser_dispatch` passed. Existing dispatch coverage still proves installed Alpha extraction, ungranted Bravo isolation, source reparse, target deletion/restoration, and configuration revocation.
- A worker-digest mismatch left the daemon responsive with degraded status and doctor output; repairing the grant and restarting restored healthy parser status. A project with a declared parser but no installation reported `PARSER_NOT_INSTALLED`; installing the package and restarting produced its edge and cleared unavailable health.
- A sandboxed fixture returned a malformed observation twice, reached broker quarantine, and reported degraded status, a fail doctor check, and `ADAPTER_QUARANTINED`. Rewriting and reconciling the source cleared quarantine, produced the guarded edge, and restored healthy status. The diagnostic file contained failure and recovery transition labels but neither the malformed source text nor the absolute project root.
- A separate three-run one-host parser cost fixture observed 15 reparses publishing in 1,121–1,159 ms after the retry fix, compared with approximately five seconds before it. This is an exploratory workstation sample, not a supported-tier release budget.

## Limits

Health is aggregated across local projects to avoid cross-project identifiers in system-wide status. It does not yet provide a project-scoped parser repair view. The installation file still requires a daemon restart after edits; there is no public installation UI or command. Quarantine recovery requires a new indexed source digest or restart, and the broker still rechecks the artifact and sandbox before delivery. Parser counters are process-local and are not a durable incident history. `host.json` records recovery state at daemon startup, so clients use live `system.status` or `system.doctor` for current parser health after a failure or recovery. No real tool adapter, minimum/recommended hardware tier, or creator-app contention test is established by this slice.
