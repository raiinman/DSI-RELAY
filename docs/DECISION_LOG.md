# Decision Log

Approved decisions captured from project planning. This file records durable outcomes, not discussion history.

## D-001 — Provider-independent core

Status: Approved

RELAY must not depend on a specific AI provider. ChatGPT, Codex, Claude, local models, and future clients are callers.

## D-002 — Multi-project product

Status: Approved

RELAY is not a tool for one map or project. Projects are isolated workspaces and project-specific behavior must not leak into the core.

## D-003 — Public-release posture

Status: Approved

Architecture, configuration, paths, documentation, and packaging decisions must be compatible with eventual public use. Public release timing and license remain undecided.

## D-004 — RELAY Core owns behavior

Status: Approved

Business logic lives once in RELAY Core/Command Bus. Interfaces call it; they do not reimplement it.

## D-005 — CLI/headless is canonical for local execution

Status: Approved

Meaningful operations must be invocable non-interactively. CLI is the preferred local interface for humans, scripts, CI, and AI clients with shell access.

## D-006 — Dashboard is first-class but not exclusive

Status: Approved

The dashboard provides observability, approvals, history, diagnostics, tests, assets, integrations, and usage views. No operation may exist only in the dashboard.

## D-007 — Cost efficiency is a primary objective

Status: Approved

Design must minimize model tokens, paid-model dependence, remote calls, cloud cost, redundant work, and unnecessary latency without sacrificing correctness, safety, or recoverability.

## D-008 — Deterministic work precedes AI work

Status: Approved

If software can measure, parse, validate, index, diff, filter, deduplicate, summarize structurally, or execute a task reliably, RELAY should do it locally before using model reasoning.

## D-009 — Compact skills are preferred for local AI

Status: Approved direction; client default requires benchmark evidence

Small skills and shell/CLI wrappers remain the leading local-AI design because they can avoid large always-loaded tool catalogs. However, recent tool-interface research shows that compact dynamically discovered MCP/tool surfaces can also be efficient. RELAY must benchmark CLI+skill against thin/dynamic MCP for each supported client class before declaring the default.

## D-010 — MCP is a thin compatibility layer

Status: Approved

MCP remains an adapter rather than RELAY's business-logic foundation. Thin capability discovery/execution/result patterns are preferred over a huge static catalog, but the exact MCP surface must be benchmarked and may vary by client.

## D-011 — Cloud-only clients use a thin remote path

Status: Approved

Normal cloud ChatGPT cannot be assumed to execute local shell commands. A remote gateway/connector may forward structured work to local relayd without moving core behavior to the cloud.

## D-012 — Full output stays in RELAY

Status: Approved with lifecycle requirement

Raw logs, large result sets, screenshots, telemetry, and historical evidence stay in RELAY storage rather than being dumped into AI context by default. Evidence is subject to explicit retention classes, quotas, privacy/secret handling, deduplication, pinning, export, and deletion policy; "recoverable" does not mean "retain forever."

## D-013 — Context Compiler

Status: Approved direction with adversarial constraints

RELAY will compile task-specific context from structured state, history, evidence, summaries, current goal, client capability, and a context/token budget. Retrieval must consider provenance, contextual intent, freshness/version, trust level, conflicts, and memory quality—not semantic similarity alone. Context eviction is not data deletion.

## D-014 — Structured exact data is protected from lossy summarization

Status: Approved

IDs, paths, coordinates, versions, numerical measurements, error codes, transaction references, and other precision-critical fields remain exact. Narrative/repeated material may be summarized or deduplicated.

## D-015 — Results are referenceable

Status: Approved

Meaningful operations should return durable result IDs so new chats and clients can retrieve evidence without rerunning work or carrying old conversations.

## D-016 — Transactions, verification, and rollback

Status: Approved

RELAY-controlled writes should record before/after state, verify actual post-write state, and expose rollback where technically possible.

## D-017 — Automation with explicit boundaries

Status: Approved with human-factors constraints

Observation, analysis, validation, indexing, narrowly scoped internal repair, and reconnection may be automatic. Destructive operations, publishing, credential changes, and spending require explicit approval. Project-content write policy must be configurable. Approval design must avoid consent fatigue through risk-adaptive batching/flight plans, and repeated self-repair failures must escalate rather than continue silently.

## D-018 — UEFN/Fortnite is the first integration

Status: Approved

UEFN/Fortnite is the first production target. Blender and Krita are companion asset integrations. The core remains engine-agnostic.

## D-019 — Runtime instrumentation matters

Status: Approved direction

The first integration should include structured runtime telemetry, probes, gameplay assertions, visual/debug overlays where supported, repeatable captures, and state verification rather than relying only on editor automation. RELAY should prefer supported engine/runtime instrumentation and normalize its output rather than creating unnecessary parallel mechanisms.

## D-020 — Usage observability is a product feature

Status: Approved

Jobs and aggregate views should measure model usage when available, remote calls, context sent/returned, local operations, cache/index reuse, elapsed time, and other metrics needed to prove RELAY saves work.

## D-021 — DOX governs repository documentation and future code

Status: Approved

AGENTS.md hierarchy is binding. Durable boundaries require appropriate DOX ownership and index maintenance.


## D-022 — Project and tool content is untrusted data

Status: Approved

Files, documentation, logs, repository text, web-derived material, assets, telemetry, and tool output may contain hostile or accidental instructions. RELAY must preserve an instruction/data boundary, attach provenance/trust metadata, and constrain actions through structured commands and permissions. Retrieved project content must never automatically become higher-authority instructions.

## D-023 — Agents and clients require attributable scoped identity

Status: Approved direction

Where integrations permit it, RELAY should identify the requesting client/agent separately from the human user, use scoped/delegated and revocable authority, avoid shared long-lived credentials, and preserve requester/delegator identity in audit records. Local workers should run with the minimum practical authority and sandboxing.

## D-024 — Evidence has a lifecycle

Status: Approved

Persistent evidence must have retention classes, project/storage quotas, deduplication/content identity, sensitive-data rules, pin/keep behavior, export/delete semantics, and migration behavior. Context removal remains separate from evidence deletion, but unlimited retention is not a product requirement.

## D-025 — Native-tool-first integration

Status: Approved

When a supported engine/tool already has an authoritative validator, profiler, session inspector, transaction system, or measurement source, RELAY should integrate and normalize it before building a competing implementation. RELAY-specific checks should focus on cross-tool correlation, project history, context, automation, and gaps in native capabilities.

## D-026 — Human situation awareness is a product requirement

Status: Approved direction

RELAY must show current automation mode, authority, active work, significant changes, and recovery options clearly enough to reduce automation surprise and out-of-the-loop operation. Warning/approval systems must be designed to limit false-alarm and consent fatigue.

## D-027 — Product defaults require reproducible benchmark evidence

Status: Approved

Benchmarks used to choose client interfaces, context strategies, models, or cost-saving defaults must define the measurement objective, baseline, versions, workload, protocol/settings, repetitions where relevant, cost controls, variation/uncertainty, and limitations. A single successful demo is not sufficient evidence.

## D-028 — UEFN MCP is a volatile adapter surface

Status: Approved

Unreal MCP/UEFN MCP must be capability- and version-gated behind the UEFN adapter. RELAY Core may not depend on its current schemas or assume the feature is complete/stable.

## D-029 — Phase 0 remains open during decision revalidation

Status: Approved

Implementation research may continue, but Phase 1 stack choices must not become durable architecture decisions until the Phase 0 adversarial-review exit criteria are satisfied.


## D-034 — Local deterministic work and local model inference are separate decisions

Status: Approved

RELAY keeps deterministic parsing, indexing, filtering, validation, and similar work local by default. Model inference location is not assumed; local, cloud, and hybrid execution must be selected from measured quality, latency, privacy, utilization, and cost.

## D-035 — File notifications are accelerators, not authoritative state

Status: Approved

Incremental file watching reduces work but can miss change details. RELAY requires reconciliation after continuity gaps and on startup, and indexing must be replay-safe.

## D-036 — Operational indexes are rebuildable derived state

Status: Approved direction

RELAY's project indexes and normalized state do not replace the underlying project or authoritative engine/runtime state. Phase 1 storage design must support integrity checking, migration, backup/recovery, and rebuild.


## D-037 — Interactive integrations require user-session hosting

Status: Approved direction

The primary Windows host must be compatible with the signed-in user's UEFN, Blender, Krita, and related tool sessions. Phase 1 will compare per-user background-host options before fixing the process model.


## D-039 — Update delivery must preserve recoverability

Status: Approved direction

Public update design must verify release integrity and preserve user project data when an update is interrupted or rolled back.


## D-040 — Third-party adapters are isolated from RELAY Core

Status: Approved direction

Public third-party adapters should run out of process by default behind a brokered, versioned protocol. Untrusted adapter code must not be loaded directly into RELAY Core.

## D-041 — Open marketplace distribution is deferred

Status: Approved

An open public adapter marketplace is not required for early RELAY releases. Explicit local installation and curated distribution are preferred until adapter identity, provenance, permissions, update, compatibility, and review controls are proven.

## D-042 — Adapters use capability manifests

Status: Approved direction

Adapters declare the minimum project, tool, network, credential, subprocess, and AI-facing capabilities they need. RELAY policy grants a bounded subset and adapters cannot expand their own authority.

## D-043 — Adapter trust is multidimensional

Status: Approved

Publisher identity, artifact integrity, build provenance, review status, granted permissions, and runtime behavior are separate trust facts. A valid signature or curation badge must not be represented as proof that an adapter is safe.

## D-044 — Adapter-provided text remains untrusted data

Status: Approved

Third-party command descriptions, documentation, errors, results, and metadata cannot become RELAY policy or higher-authority AI instructions. AI-facing descriptions are normalized/rendered by RELAY from reviewed command semantics and bounded adapter metadata.

## D-045 — Adapter distribution is exact-version and compatibility gated

Status: Approved direction

Adapter artifacts should be exact-version pinned and integrity-identified, with dependency/component inventory, compatibility declarations, update history, and rollback/recovery support. Incompatible adapters fail closed or are quarantined rather than loaded optimistically.


## D-046 — Out-of-process and sandboxed are different states

Status: Approved

Third-party adapters run out of process by default for fault isolation. RELAY must not describe that as a security sandbox unless OS-enforced or equivalent resource restrictions are actually active.

## D-047 — Adapter trust includes companion components

Status: Approved direction

If an integration installs code inside a target application or ships helper/runtime components, those artifacts are part of the adapter's supply chain and require their own version, integrity, provenance, and compatibility records where practical.
