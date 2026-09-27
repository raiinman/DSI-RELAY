# Purpose

Own the versioned machine contracts shared by RELAY Core and every client surface.

# Ownership

- Governs `crates/relay-contracts/`.
- Contains no storage, IPC, UI, adapter, or product-specific business logic.

# Local Contracts

- Own versioned command request/result/error envelopes and protocol constants.
- Own the production command registry and deterministic bounded JSON Schema validation profile.
- Stable command/capability IDs are never silently reused.
- Additive optional fields remain compatible; incompatible requested command versions fail explicitly.
- Discovery metadata must remain compact enough for CLI/dashboard/AI clients.
- Project configuration commands expose versioned, project-scoped metadata. A declared adapter ID/version is a binding preference, never proof that the adapter is installed or that its parser output is trusted.
- Project lifecycle commands expose active, archived, and removed states. Archive and restore preserve history; removal requires an exact project-ID confirmation and retains a tombstone.
- Adapter-only parser operations use the shared bounded request/result schema, but are worker observations rather than Core state-write commands. Only Core's project-scoped dependency replacement can commit edges.
- Dependency replacement accepts an optional complete configuration-selection guard (revision, adapter ID, adapter version). Core checks it atomically for automatic parser submissions; omitted guards preserve existing manual callers and grant no authority.
- `system.status` permits an additive bounded host-component health array; command clients can read live parser health without reading local host state files.
- `result.describe` is a distinct read-only command for metadata and stored payload size; `result.get` remains the full-payload command.
- `result.list` returns only bounded result descriptions, never payload or provenance; scoped clients must supply an authorized project ID.
- `job.list` returns bounded job descriptions without checkpoint or provenance; scoped clients must name an authorized project.
- `result.context` is a byte-budgeted read of exact allowlisted scalar facts by result ID. An optional bounded `required_pointers` list must return each eligible exact fact or fail explicitly; it cannot override the safety filter or project scope. The response references `result.get` for complete evidence and does not authorize remote data transfer.
- `result.context` may use bounded task focus terms to prioritize already eligible facts; terms are not echoed in the result.
- `context.compile` selects at most eight same-project stored results under one byte budget, requires exact source-qualified pointers, carries result IDs, kinds, and source freshness/trust metadata, and emits all eligible same-kind/same-pointer conflicts or fails for insufficient budget.
- `usage.summary` exposes additive current-process `context_cache` counters for actual reuse and skipped compiler fact collection, separate from persisted command/model-token metrics. `system.status` may expose the same bounded cache summary.
- `diagnostics.summary` exposes bounded diagnostic health and aggregate counts without raw log history or caller-supplied text.
- `automation.pause` and `automation.resume` are host-control commands for optional watcher reconciliation in the current daemon run. Status and diagnostic summaries expose `running`, `paused`, or `unavailable` without changing overall process health.
- `uefn.static.inspect` is an observe-only project-scoped command. It classifies a ready indexed project and explicitly leaves live editor/runtime capabilities unchecked.
- `assets.manifest.validate` is a project-scoped local analysis command over a bounded manifest. It reports source/export link findings without claiming Blender, Krita, or UEFN execution.
- `assets.impact.analyze` accepts a bounded manifest and changed project-relative paths, validates structure and project scope, and returns deterministic affected asset IDs and reason codes. Missing or stale local files remain valid impact inputs; creator-app execution stays not checked.
- `assets.blender.mesh.validate` accepts one authorized project-relative `.blend` file and returns bounded sanitized geometry findings with an explicit native workflow status; unavailable Blender never becomes a pass.
- `assets.krita.inspect` reuses that bounded manifest and reports declared Krita formats and local links; native Krita export/content checks stay untested.
- `assets.krita.export` is a CLI-only explicit `project_write` action over one project-relative `.kra` and a new `.png`; it never accepts an executable or script path, and reports native unavailability as untested.
- `runtime.capture.analyze` normalizes bounded caller-supplied structured Verse logs and local assertions while keeping live UEFN verification explicitly untested.
- `uefn.mcp.discover` probes a user-selected localhost port for a bounded MCP tool catalog; a response is discovery evidence, not editor identity or workflow proof.
- `uefn.mcp.toolsets` queries only advertised discovery tools and returns bounded names; live editor identity and workflows remain untested.
- `uefn.mcp.describe_toolset` summarizes only advertised input parameter names/types/required flags; it never dispatches editor actions or echoes tool descriptions.
- No secret values, project paths, or runtime state are embedded in contract metadata.

# Verification

- Registry self-validation.
- malformed arguments/results fail deterministically.
- compatibility tests for optional additions, required-field changes, and version rejection.
- compact discovery tests.

# Child DOX Index

- No child DOX files currently exist.
