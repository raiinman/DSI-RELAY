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


## D-048 — Remote AI is an explicit data-processing boundary

Status: Approved

Remote model, embedding, gateway, and support destinations are not transparent compute. Project data may leave only when destination/project policy allows it, and outbound processing should be attributable.

## D-049 — Credentials never become model context

Status: Approved

Models receive opaque connection/credential references. Secret values are resolved only by trusted execution components that require them and are excluded from prompts, embeddings, summaries, ordinary logs, and AI memory.

## D-050 — Derived artifacts inherit sensitivity

Status: Approved

Embeddings, summaries, screenshots, cached outputs, and other derivatives inherit relevant source sensitivity by default and retain provenance unless an explicit policy transformation changes their classification.

## D-051 — Read authority and egress authority are separate

Status: Approved

Permission to inspect project data does not grant permission to transmit it to remote models, networked adapters, support systems, or other processors. Egress is task-, project-, and destination-scoped.

## D-052 — Context policy precedes context optimization

Status: Approved

The Context Compiler first determines what data is eligible for the destination, then optimizes relevance and token budget inside that allowed set. Relevance cannot override privacy policy.

## D-053 — Local-only/private mode requires verifiable semantics

Status: Approved direction

RELAY may offer project modes that prohibit remote project-data processing, but such labels require black-box/network verification and documented exceptions. Local-only project data and completely offline product operation are distinct concepts.

## D-054 — Provider privacy metadata is versioned external state

Status: Approved direction

RELAY records what is known about a remote processor's data-use, retention, residency, and account/endpoint context together with source and verification time. Unknown or stale properties remain unknown.

## D-055 — AI outputs inherit data policy

Status: Approved

Responses that reproduce or derive from sensitive source material remain governed by relevant project sensitivity, retention, share/export, and onward-egress policy.


## D-056 — Remote AI does not receive unrestricted project access

Status: Approved

Remote AI clients access project data through RELAY retrieval, classification, and egress policy. Connecting a remote client does not grant raw filesystem access by default.

## D-057 — Metadata participates in data classification

Status: Approved

Paths, filenames, project/repository names, asset names, document metadata, identifiers, timestamps, and similar metadata may be sensitive and are covered by project egress policy where applicable.

## D-058 — Privacy/secret scanning is defense-in-depth

Status: Approved

A clean scanner result does not declassify project data. Explicit project/path/artifact policy and provenance can impose stricter sensitivity than automated detectors.


## D-059 — Human, client, agent, and service identities remain distinct

Status: Approved

RELAY should preserve useful actor/delegator attribution instead of collapsing important activity to one shared bot or human identity.

## D-060 — Roles are UX templates, not the entire authorization model

Status: Approved direction

Simple roles may simplify administration, but enforcement must be able to consider project/resource, action, client/agent scope, command risk, data policy, and other relevant attributes.

## D-061 — Delegated authority cannot silently widen

Status: Approved

Agent-to-agent or client-to-agent delegation must preserve or narrow the authority, project/resource scope, data-egress scope, and relevant budgets of the delegating principal.

## D-062 — Membership and external connection ownership are separate

Status: Approved

Removing a person from a workspace does not prove that separately shared external credentials or service connections are revoked. Offboarding must evaluate both membership and connection ownership.

## D-063 — Team writes require stale-state protection

Status: Approved

Important write plans and approvals should carry project revision or equivalent preconditions. If relevant state changes before execution, RELAY should stop and re-inspect rather than silently apply a stale plan.

## D-064 — Multi-agent work is coordinated and budgeted, not swarm-by-default

Status: Approved direction

RELAY may support multiple agents, but coordination, authority, context, and cost must be explicit. The default should use the smallest number of agents that measurably improves the task.

## D-065 — Revocation propagates to active work

Status: Approved direction

Identity/client/agent revocation should re-evaluate active sessions, queued jobs, approvals, delegated work, resource claims, and relevant connection use instead of affecting only future sign-ins.


## D-066 — Durable execution and external-effect safety are separate

Status: Approved

Persisting/replaying a job does not guarantee an external side effect occurred exactly once. Side-effecting commands require explicit retry/idempotency/unknown-outcome semantics.

## D-067 — Unknown outcome is a first-class operation state

Status: Approved

If RELAY cannot tell whether an external effect completed, it records uncertainty and reconciles the external source of truth before repeating a non-idempotent action.

## D-068 — Rollback, compensation, restore, and manual recovery are distinct

Status: Approved

RELAY must describe the actual recovery mechanism and cannot label compensation or a later inverse action as an atomic rollback.

## D-069 — Backups require restore verification

Status: Approved

For RELAY-owned durable state, a created backup is not considered proven recovery until integrity/restore procedures have been tested.

## D-070 — Durability targets vary by state class

Status: Approved direction

Rebuildable indexes/caches, active workflow checkpoints, transaction/approval state, configuration, and raw evidence have different loss/recovery tolerances and should not share one blanket durability policy.

## D-071 — Binary rollback and data rollback are separate

Status: Approved

Application downgrade/update rollback does not guarantee newer-format RELAY data is backward compatible. Schema/data migration requires independent recovery planning.

## D-072 — Recovery state is separate from process health

Status: Approved

RELAY may run while still Reconciling, Degraded, Blocked, or awaiting Manual Recovery. Healthy status requires appropriate integrity/current-state verification.

## D-073 — Fault injection is required for recovery validation

Status: Approved direction

Crash, duplicate-delivery, network-loss, disk-write failure, migration interruption, and similar fault scenarios belong in implementation testing rather than being treated only as theoretical risks.

## D-074 — Remote outages degrade capability without silently changing data policy

Status: Approved

Failure of a remote AI/provider/gateway should preserve local deterministic functionality where possible and must not silently switch to another processor with different privacy or data-use policy.


## D-075 — Observability is evidence, not ground truth

Status: Approved

RELAY distinguishes authoritative state, observation, derived measurement, corroborated finding, inference, and unknown evidence rather than presenting every signal as fact.

## D-076 — Missing or sampled telemetry is not negative proof

Status: Approved

Absence of an event is interpreted as absence only when the collection contract is known complete for that event. Sampling, drops, disconnects, parser failure, retention, and staleness remain visible evidence limitations.

## D-077 — Instrumentation has an explicit overhead budget

Status: Approved direction

Probes/tracing/diagnostics must be benchmarked for CPU, latency, storage, network, and project/tool impact. High-detail instrumentation is targeted rather than assumed free.

## D-078 — Event causality does not rely on wall-clock order alone

Status: Approved

RELAY prefers causal/sequence relationships over timestamp ordering when diagnosing event chains across components.

## D-079 — Detector quality includes operational alert burden

Status: Approved

False alarms, duplicate/noisy findings, calibration/confidence, and user-operational burden matter in addition to benchmark accuracy/precision/recall.

## D-080 — The observability pipeline has its own health state

Status: Approved

Collector/adapter connectivity, dropped events, parser failures, sampling, backlog, storage pressure, and staleness can reduce confidence in downstream findings.

## D-081 — Root-cause language reflects evidence strength

Status: Approved

RELAY distinguishes symptoms, correlations, hypotheses, tested hypotheses, and verified causes; correlation alone is not presented as proof.

## D-082 — Visual evidence is revision/session aware

Status: Approved

Screenshots/captures retain project/session/revision/view metadata and are not substituted for hidden structured state.

## D-083 — Product metrics use quality guardrails

Status: Approved

Token reduction, AI-call avoidance, alert count, cache hit rate, and similar metrics cannot be optimized alone when doing so harms correctness, freshness, safety, or usability.

## D-084 — Observability metadata is untrusted input

Status: Approved

External trace/correlation IDs, log metadata, and diagnostic control fields cannot grant authority or force unbounded collection; they are validated and resource-limited.


## D-085 — Simplicity is a first-class product constraint

Status: Approved

RELAY is evaluated not only for capability, security, and correctness but also for cognitive load, onboarding burden, configuration burden, diagnostic effort, and maintenance complexity.

## D-086 — Safe defaults are part of product security

Status: Approved

Public users should not need to design their own security/privacy/recovery architecture before first use. Safe project isolation, credential handling, egress, retry, retention, and adapter defaults should be opinionated and usable out of the box.

## D-087 — Choice design is contextual

Status: Approved

RELAY does not assume fewer options are always better. It uses strong defaults, contextual choices, search/filter, recommendations, and progressive depth according to task difficulty and user uncertainty.

## D-088 — Configuration options consume a test/support budget

Status: Approved

Every user-visible configuration option is additional product state with testing, migration, documentation, interaction, and support cost. Internal knobs do not automatically become public settings.

## D-089 — Important configuration is validated early

Status: Approved

RELAY should run startup/preflight checks for important configuration and integration assumptions where practical rather than waiting for rare paths or failures to expose invalid settings.

## D-090 — Human verification load is part of AI cost

Status: Approved direction

A workflow that saves model tokens but forces repeated manual verification can still be expensive. AI/client UX benchmarks include human review, approvals, correction, and fatigue where practical.

## D-091 — Core features require admission and retirement discipline

Status: Approved

A feature enters RELAY Core only when its user value justifies long-term implementation, test-matrix, security/privacy, migration, documentation, runtime, and support cost. Removal, merge, and demotion are valid product improvements.

## D-092 — Generic adapter abstractions are evidence-driven

Status: Approved direction

The first real integration should not be forced into an imagined universal engine model. Core abstractions remain minimal until validated against materially different integrations.

## D-093 — RELAY has an operability/toil budget

Status: Approved direction

Repetitive reconnection, reconfiguration, update repair, permission cleanup, compatibility triage, reindexing, approval, and support tasks are measured and targeted for automation or root-cause removal.

## D-094 — Common diagnostics have one low-friction entry point

Status: Approved direction

A future relay doctor/dashboard equivalent should summarize what is wrong, what still works, what RELAY already tried, and the next useful action without requiring users to understand internal architecture.

## D-095 — Personal UI does not inherit enterprise complexity

Status: Approved

The data model may support future team/organization capabilities, but normal personal onboarding and project workflows hide SSO, custom roles, organization policy, audit administration, and similar concepts until relevant.

## D-096 — Basic value does not require remote AI

Status: Approved

A supported personal project should produce a useful deterministic local scan/audit before the user configures cloud AI, team identity, custom adapters, or advanced policy.

## D-097 — Complexity regression can fail a milestone

Status: Approved direction

Milestones track time/steps to useful value, mandatory decisions/concepts, configuration growth, background components, compatibility surface, support/diagnostic effort, and similar measures. Functional completeness alone does not excuse an unacceptable complexity regression.

## D-098 — Inactive integrations stay operationally quiet

Status: Approved

Adapters/features not relevant to the active project/task should contribute negligible routine AI context and should avoid unnecessary background CPU, memory, network, health polling, and dashboard noise.


## D-099 — UEFN automation stays on documented developer surfaces

Status: Approved

RELAY's UEFN/Fortnite integration uses Epic-documented developer automation such as UEFN MCP and supported development/test interfaces. This does not authorize general Fortnite gameplay/client automation, integrity-system circumvention, or undocumented botting.

## D-100 — Published UEFN content does not phone home to RELAY

Status: Approved

Runtime instrumentation for published UEFN projects must use supported Epic/UEFN mechanisms and may not introduce external RELAY server connections prohibited by current UEFN terms.

## D-101 — RELAY branding is independent of Epic IP

Status: Approved

Public RELAY naming, logos, and product identity do not depend on Epic/Fortnite/Unreal marks or imply endorsement. Platform names are used descriptively for compatibility where appropriate.

## D-102 — Third-party creative tools are detected rather than bundled by default

Status: Approved direction

Public installers should detect user-installed UEFN, Blender, Krita, and similar tools rather than redistributing them unless a specific release accepts and satisfies the applicable license/trademark/update obligations.

## D-103 — Companion components may have different licenses from RELAY Core

Status: Approved

Blender/Krita companion plugins and Unreal/UEFN components are licensed/distributed according to their host-platform obligations. RELAY Core's eventual license does not automatically apply to every companion.

## D-104 — Project assets carry rights/provenance metadata

Status: Approved direction

The asset/content model supports creator/source, license/rights basis, Epic/third-party status, attribution, redistribution/export constraints, and AI-generated/assisted provenance where useful.

## D-105 — RELAY does not promise copyright ownership of AI output

Status: Approved

RELAY separates provider contractual usage rights, provenance, human authorship/modification, and copyrightability. UI/docs do not claim prompts or AI generation automatically confer copyright or non-infringement.

## D-106 — Public artifacts require license/component inventory

Status: Approved direction

Distributed RELAY artifacts need reproducible dependency/component/license inventories and required notices/source obligations before public release.

## D-107 — Platform terms and licenses are versioned external dependencies

Status: Approved

Material Epic rules, licenses, AI-provider terms, and other integration policies are tracked with source/update/review metadata. Material changes trigger compatibility review rather than silent assumption.

## D-108 — Automated compliance findings are advisory controls

Status: Approved

RELAY may identify missing metadata, known conflicts, or policy drift, but does not represent automated checks as legal certification, copyright clearance, or platform approval.

## D-109 — RELAY Core license selection remains open

Status: Approved

The Core/SDK/companion license strategy is selected only after integration boundaries, GPL companion obligations, Unreal/UEFN restrictions, contributor policy, and commercial/hosted goals are prototyped/reviewed.

## D-110 — Public privacy/security claims must match tested behavior

Status: Approved

Marketing, onboarding, telemetry/privacy notices, and UI claims such as local-only/private/encrypted/no-upload must remain consistent with actual tested behavior and documented limitations.


## D-111 — Version numbers do not substitute for compatibility tests

Status: Approved

RELAY may use Semantic Versioning or another release-number policy, but automated compatibility testing and explicit capability/schema contracts determine actual interoperability.

## D-112 — Breaking-change review includes behavior

Status: Approved

A breaking change can include command side effects, error/exit semantics, permission requirements, idempotency/retry behavior, privacy/egress defaults, ordering/pagination, and material performance/rate/concurrency contracts—not only renamed or removed fields.

## D-113 — Mixed-version operation is expected

Status: Approved

CLI, local host, dashboard, gateway, adapters, companions, skills, and stored data may be on different supported versions. Each durable interface defines supported skew and fails safely outside it.

## D-114 — Serialization format defines schema-evolution rules

Status: Approved direction

RELAY's eventual IDL/serialization choices must be accompanied by explicit forward/backward compatibility rules for every supported representation. Safe evolution in one encoding is not assumed safe in another.

## D-115 — Historical results keep producing-version semantics

Status: Approved

Durable results retain producing RELAY/schema/command/rule/adapter version metadata. New code must not silently reinterpret old result fields using changed semantics.

## D-116 — Deprecation is a managed lifecycle

Status: Approved

Stable public contracts require replacement/migration metadata, owner, support/removal rules, and appropriate warning/usage signals before removal.

## D-117 — Breaking changes should ship migration assistance

Status: Approved direction

For meaningful public contract changes RELAY should provide scanners, data/config migrations, command rewrite guidance, regenerated skills/wrappers, or explicit manual steps where practical.

## D-118 — Compatibility shims have retirement criteria

Status: Approved

Aliases/shims are versioned compatibility debt with owner, tests, supported range, usage signal, and removal condition. They do not remain indefinitely by default.

## D-119 — Feature flags are lifecycle-managed compatibility state

Status: Approved direction

Flags used for rollout or compatibility have owner, introduction version, purpose/default, compatibility implications, and removal/review criteria.

## D-120 — Human CLI text is not the automation contract

Status: Approved

Scripts and AI skills use structured versioned output and documented exit-code semantics. Human presentation can evolve without becoming an accidental machine API.

## D-121 — Capability negotiation complements version negotiation

Status: Approved

Peers exchange both protocol/version compatibility and actual capabilities when available; clients do not infer every feature from release number alone.

## D-122 — Retired schema identifiers are not reused unsafely

Status: Approved

Where the chosen schema technology gives identifiers lasting wire/storage meaning, retired IDs/names are reserved or otherwise protected from reuse according to that technology's evolution rules.

## D-123 — Security fixes may explicitly break compatibility

Status: Approved direction

Serious security/privacy flaws can justify a breaking change, but the compatibility impact, affected versions, mitigation, migration, and support exception must be communicated explicitly.

## D-124 — Public compatibility support is bounded

Status: Approved direction

Before stable public release RELAY defines realistic release/API/SDK/data-migration/skill support windows rather than promising indefinite compatibility.

## D-125 — Contracts have stability classes

Status: Approved direction

Internal, experimental, preview, and stable surfaces carry different compatibility/deprecation promises so immature interfaces are not frozen prematurely.

## D-126 — Legacy compatibility stays off the normal context path

Status: Approved

Deprecated/legacy commands, schemas, and skill descriptions remain available only to clients/migrations that need them and do not inflate ordinary AI context or dashboard complexity.


## D-127 — Foreground creator workloads outrank optional RELAY work

Status: Approved

Active UEFN/Fortnite/Blender/Krita interaction has priority over deep indexing, maintenance, high-detail telemetry, and optional local AI/model workloads.

## D-128 — Background work must adapt to contention

Status: Approved direction

RELAY background jobs should use OS-supported QoS/priority/backoff mechanisms where useful and yield when foreground resource pressure rises.

## D-129 — Useful readiness is progressive

Status: Approved

A project can become useful before deep/content/semantic indexing completes. Basic local audit and status should not wait on optional expensive indexing stages.

## D-130 — Windows indexing should evaluate USN-assisted incrementality

Status: Approved direction

On NTFS, RELAY should evaluate USN journal/change-feed acceleration combined with baseline/reconciliation scans instead of repeated full-tree polling.

## D-131 — Inactive projects have an idle resource budget

Status: Approved

Adding projects must not permanently add full resident workers, loaded models, semantic indexes, or heavy polling. Idle cost per project is measured.

## D-132 — Local AI routing includes GPU/VRAM contention

Status: Approved

Local inference decisions consider active creator workload, GPU/VRAM/memory pressure, and foreground responsiveness. Saving remote tokens does not justify materially degrading the editor/play session.

## D-133 — Interactive tail latency is a performance contract

Status: Approved

Performance acceptance includes user-visible p95/p99/tail latency and foreground responsiveness, not only average throughput or average CPU.

## D-134 — Database/index maintenance has a resource budget

Status: Approved direction

Checkpoint, compaction, vacuum, migration, and related maintenance are benchmarked for latency, disk amplification, concurrency, and interruption/recovery impact and are deferred around active work where practical.

## D-135 — Product storage limits are below technology maxima

Status: Approved

RELAY uses project/global quotas and supported-size limits appropriate to public hardware instead of relying on theoretical database/filesystem maximums.

## D-136 — Large evidence uses tiered storage

Status: Approved direction

Screenshots, profiler captures, telemetry dumps, and other large binaries are retained according to policy using metadata references, compression/deduplication, and file/object-style local storage where that benchmarks better than database BLOBs.

## D-137 — OS-native resource controls are evaluated first

Status: Approved direction

Windows EcoQoS, Job Objects, process/thread priority, and memory-priority mechanisms should be benchmarked before RELAY builds custom resource scheduling.

## D-138 — Resource modes are primarily automatic policy

Status: Approved direction

Foreground-safe, balanced, idle/batch, and diagnostic resource modes may exist internally, but normal users should not need to tune low-level scheduler controls.

## D-139 — Hardware-tier benchmarks are mandatory

Status: Approved

Performance is tested against versioned minimum-class, recommended-class, and high-end creator fixtures rather than one developer workstation.

## D-140 — Resource-intensive jobs declare scheduling characteristics

Status: Approved direction

Jobs can declare foreground/background priority, expected CPU/RAM/GPU/I/O intensity, urgency, interruptibility, and active-project association so the scheduler can defer lower-value work safely.

## D-141 — Performance telemetry is tiered

Status: Approved

Normal mode uses low-overhead resource summaries; high-resolution counters/tracing are temporary diagnostic/benchmark tools and are not retained indefinitely by default.

## D-142 — Energy is measured but not overgeneralized

Status: Approved

Power/battery/thermal impact may be part of RELAY benchmarks, but national data-center totals or unrelated hardware studies are not used to declare local or cloud inference universally more efficient.

## D-143 — Local resource regression can fail a release

Status: Approved

AI-token savings do not excuse startup, idle-memory, storage, foreground-latency, or maintenance regressions beyond the supported resource budget.

## D-144 — Long-run performance aging is tested

Status: Approved direction

Soak tests track database/index growth, evidence retention, memory leaks, stale workers/watchers, maintenance frequency, and performance after long project histories.

## D-145 — Performance cost is part of the same cost mission as AI usage

Status: Approved

RELAY's cost model includes local CPU/GPU/RAM/disk/network/power and human-visible latency. "Cheaper AI" is not success if the local workstation pays a larger resource cost.

## D-146 — Node per-user host remains a provisional Phase 1 candidate

Status: Superseded by D-152 and D-153

Spike 1 proved a normal signed-in-user Node 24.19 process can host the structured RELAY contract over a Windows named pipe with authenticated version/capability negotiation, non-interactive CLI execution, diagnostics, and hard-kill restart recovery.

On the 2026-09-24 Windows fixture using the integrated Spikes 1–3 source snapshot, startup-to-ready was 91.536 ms p50, authenticated handshake + status was 1.058 ms p50, and a full CLI process + status was 141.929 ms p50. The host sampled 118,153,216 bytes RSS while idle.

Node may continue through Phase 1 because it gives a dependency-free prototype path to SQLite and Zstandard on the current fixture. It is not the final runtime choice: idle memory is material, and explicit Windows named-pipe DACL/cross-user denial has not yet been proven. Final host/IPC selection requires a lower-footprint comparison and OS access-control tests.

Evidence: `spikes/phase1/results/2026-09-24-spike1-node-windows.json`.

## D-147 — SQLite is the provisional operational-state store

Status: Confirmed and selected by D-153

Node's built-in SQLite 3.53.3 in WAL mode with `synchronous=FULL` is approved to continue as the prototype store for project registry data, compact results, job/checkpoint state, and schema migration metadata.

Spike 2 measured 2.013 ms p50 durable result writes, 1.136 ms p50 result reads, and 1.164 ms p50 `quick_check` round trips. Results and checkpoints survived a hard process kill, and deliberately damaged storage caused a `Degraded` host with blocked writes rather than false health.

This decision does not approve heavyweight evidence BLOBs in SQLite. Concurrency, checkpoint starvation, VACUUM/compaction, disk-full injection, backup/restore, and interrupted migration remain required before the storage architecture is final.

Evidence: `spikes/phase1/results/2026-09-24-spike2-node-sqlite-windows.json`.

## D-148 — Evidence reduction is a scoped pipeline, not maximum compression

Status: Phase 1 provisional

Spike 3 supports this prototype direction:

- hash exact bytes before storage and deduplicate whole blobs before compression
- use project-scoped deduplication by default; workspace-scoped deduplication requires an explicit policy because it crosses project boundaries
- use fast Zstandard, currently level 1, as the hot/warm exact-evidence candidate only when compression actually reduces bytes
- store incompressible payloads raw rather than paying CPU to make them larger
- keep heavyweight evidence as hashed files/blobs with SQLite metadata unless later real-workload evidence reverses the maintenance tradeoff
- treat log aggregation and telemetry downsampling as retention-class transformations, never invisible replacements for exact evidence
- allow colder idle-time recompression only when a measured break-even justifies the extra CPU/I/O
- keep lossy image encodings as reference derivatives; pinned/exact evidence requires lossless storage

On the synthetic lifecycle corpus, project-scoped whole-blob dedupe removed 71.7% of raw referenced bytes before compression; workspace scope removed 80.0%. Level-19 Zstandard took roughly 6.8–9.4 seconds on the ~9–12 MB structured-text fixtures for relatively small gains over fast levels, while incompressible data grew slightly at every tested level. Recompressing the layout corpus from level 1 to level 9 saved only 0.3573% more bytes for 143 ms of work.

SQLite BLOBs were faster on the small layout fixture, but `VACUUM INTO` required another database-sized copy. Metadata + hashed files had sub-millisecond-to-low-millisecond reads/writes while limiting atomic recompression headroom to one blob. The image-format result is not sufficient to pick a production codec because the synthetic image is not representative of UEFN screenshots.

Evidence: `spikes/phase1/results/2026-09-24-spike3-evidence-lifecycle-windows.json`.

## D-149 — Windows indexing uses watcher hints plus reconciliation; USN is optional privilege

Status: Phase 1 provisional

Spike 4 rejects both repeated full parsing and file notifications as standalone sources of truth.

The least-privilege Windows prototype path is:

1. establish a persisted baseline/reconciliation snapshot
2. use recursive file notifications as low-latency dirty-path hints
3. parse/hash only candidate files during normal incremental work
4. reconcile against project state on startup, after watcher downtime/errors, after uncertain bursts, and according to later resource-aware policy
5. expose Reconciling/Degraded state instead of calling an uncertain index current

On the 15,000-file synthetic fixture, a metadata reconciliation scan took 2.420 s and a full content parse took 10.562 s while reading 16,028,956 logical bytes. After 160 live file operations, changed-only parsing handled 150 files in 103.241 ms and read 154,660 bytes, a 99.0354% byte reduction versus parsing the full current tree.

Windows recursive notifications were fast but not complete under burst load. A rapid burst exposed only 36 of 170 changed paths (21.1765%). A control run with 40 edits spaced 10 ms apart detected 40 of 40 paths with 3 ms p50 latency. Therefore notification delivery is an accelerator/hint channel, not authoritative project truth.

An intentional watcher outage followed by 70 changed paths was fully recovered by reconciliation; the scan took 2.334 s and changed-only parsing of the recovered candidates took 43.399 ms while reading 61,498 bytes.

NTFS USN remains useful in principle but is not a normal-host dependency. Journal metadata was queryable in the signed-in user context, the journal advanced across offline mutations, continuity metadata remained valid, and Node file inode values matched NTFS USN file-reference IDs. However reading journal records returned Access Denied without elevation on the benchmark fixture. A future narrowly privileged helper may be benchmarked only if its recovery/latency benefit exceeds installer, security, compatibility, and operability cost. RELAY Core must remain correct without it.

Evidence: `spikes/phase1/results/2026-09-24-spike4-windows-indexing.json`.

## D-150 — Foreground-safe scheduling defers first; hard CPU caps are not a default

Status: Phase 1 provisional

Spike 5 measured RELAY-style CPU hashing/compression work while the real UEFN editor process and main window were active. RELAY does not raise or alter the creator application's priority; scheduling policy applies only to RELAY-owned work.

The provisional resource policy is:

1. detect an active creator workload and enter `foreground_safe`
2. defer or do not start optional heavy work such as deep indexing, storage maintenance, inactive-project reconciliation, and local AI
3. allow explicit bounded diagnostics or interactive RELAY commands when required
4. when background work must continue, prefer soft OS intent signals such as Below-Normal priority / EcoQoS and cooperative backoff over hard CPU caps
5. do not apply Job Object hard CPU caps as a routine foreground-safety mechanism
6. keep inactive projects cold; registering projects must not create resident workers or periodic compute by itself

On the high-end Windows fixture with UEFN open, baseline UEFN `WM_NULL` message-pump latency was about 5 ms p95. With 12 RELAY worker threads, Normal, Below-Normal, EcoQoS, weight-based Job Object, and a 25% hard cap all kept median p95 near baseline; therefore this fixture did not prove an editor-responsiveness benefit from always-on throttling.

The throughput cost was material. Normal-priority background work measured about 13.3 GiB/s hashing throughput, while Below-Normal/EcoQoS/weight-based controls generally reduced throughput into the roughly 10.4–12.1 GiB/s range. The 25% hard cap reduced throughput to roughly 6.1–6.5 GiB/s without improving p95.

A second full-CPU saturation run used all 16 logical CPUs. Normal-priority work still held UEFN p95 near the 4.955 ms saturation baseline while delivering about 14.1–14.3 GiB/s. EcoQoS reduced throughput to about 10.1–11.3 GiB/s with no meaningful p95 improvement. The 25% hard cap reduced throughput to about 6.1–6.3 GiB/s and produced much worse UEFN p99 tail latency (about 12.8–15.1 ms in the two runs versus about 5.1–5.4 ms for Normal). This rejects hard caps as the normal scheduling policy.

Registered inactive projects remained effectively cold: one project and one hundred projects both sampled 0 ms host CPU over three idle seconds, with only about 180 KB RSS difference on the fixture. This supports persisted cold state rather than one resident service stack per project.

EcoQoS, memory priority, Below-Normal priority, and Job Object controls were successfully applied and queried through Windows APIs. Memory-priority behavior under actual system memory pressure, Fortnite play-session frame time, GPU local-model contention, and minimum/recommended hardware remain open benchmark gates.

Evidence: `spikes/phase1/results/2026-09-24-spike5-resource-coexistence-windows.json`.

## D-151 — Dashboard is a thin static/HTTP surface hosted by relayd

Status: Phase 1 provisional

Spike 6 rejects a separate resident dashboard backend/process for the default personal installation.

The dashboard remains a client/presentation surface over RELAY's one command system:

- named-pipe clients and the embedded HTTP adapter use the same structured `dispatchCommand` envelope builder
- dashboard browser code owns rendering only; it does not access SQLite/project files or implement health/project/result business rules
- the Phase 1 shell exposes a read-only command subset: `system.status`, `system.doctor`, `project.list`, and `result.get`
- dashboard-only side-effect execution is not created; `system.shutdown` and other non-exposed commands are rejected by the adapter
- Core degraded/error state passes through unchanged; the dashboard does not reinterpret a damaged store as healthy
- the local HTTP listener binds to loopback and uses a per-start random same-origin dashboard token plus restrictive security headers/CSP; this is an implementation boundary, not a completed public browser threat model

On the high-end Windows fixture, the static dashboard assets totaled 9,515 bytes and added zero runtime package dependencies.

A host-only process sampled 125,329,408 bytes RSS. Running a standalone dashboard proxy plus host sampled 260,890,624 bytes RSS, or 135,561,216 bytes above the host-only run. Hosting the same dashboard HTTP/static surface inside `relayd` sampled 131,760,128 bytes RSS, only 6,430,720 bytes above host-only in this run. All three shapes sampled 0 ms CPU during their five-second post-cooldown idle windows.

Latency also favored the embedded shape. Direct named-pipe `system.status` was 0.295 ms p50 in the host-only run. The standalone dashboard proxy measured 1.356 ms p50 because it adds HTTP plus another named-pipe hop. The embedded HTTP path measured 0.378 ms p50 while still dispatching through the same command semantics.

Therefore the default personal dashboard direction is static/local web presentation served by the existing per-user host rather than another resident dashboard daemon. A future desktop wrapper may host or navigate this surface, but it must not introduce a second command/business-logic backend merely for UI packaging.

Open gates remain for write/approval authorization, client identity, live update transport, accessibility/usability testing, browser threat modeling, packaging, and public update integrity.

Evidence: `spikes/phase1/results/2026-09-24-spike6-dashboard-shell-windows.json`.

## D-152 — Rust becomes the preferred Phase 1 runtime/IPC candidate for deeper parity

Status: Superseded by D-153

Spike 7 changes the preferred runtime/IPC candidate from Node to Rust for the next parity work. It does not yet make Rust the final implementation language or approve a rewrite of every proven subsystem.

A neutral head-to-head harness used the same Windows workstation, named-pipe protocol requests, restart fault, process metrics, CLI launches, and embedded-dashboard workload for both candidates. The Rust challenger deliberately implemented only the host/runtime/IPC surface; SQLite, indexing, evidence storage, background jobs, and adapters remain unported until Rust earns them.

On the final comparison:

- Node idle host RSS was 119,095,296 bytes; Rust was 6,529,024 bytes, a 94.52% reduction.
- Node repeated startup was 91.340 ms p50; Rust was 29.991 ms p50.
- Direct authenticated `system.status` was 0.284 ms p50 on Node and 0.265 ms on Rust.
- A full CLI-process status call was 141.296 ms p50 on Node and 21.476 ms on Rust.
- After hard kill, replacement state became ready in 81.895 ms on Node and 27.251 ms on Rust; both returned healthy status afterward.
- With the same embedded dashboard assets, Node sampled 127,373,312 bytes RSS and Rust 7,036,928 bytes. Dashboard status was 0.748 ms p50 on Node and 0.555 ms on Rust.
- The actual Node CLI successfully called the Rust host, and the actual Rust CLI successfully called the Node host, proving protocol-level interoperability for the shared command subset.
- Wrong auth tokens and incompatible protocol ranges failed closed on both candidates.

Rust also closes the unresolved local-IPC access-control gate. The challenger creates the Windows named pipe with a protected current-user-only DACL and retains the per-start random application token as defense in depth. After creation it calls Windows `GetSecurityInfo` on the actual pipe handle and refuses startup unless the kernel-returned descriptor is owned by the current user, has a protected DACL, and contains exactly one full-control ACE for that user. The final benchmark reported this verification as passed. Node/libuv's pipe ACL remains unverified rather than being classified as insecure.

The trade-off is real. The selected Node host path measured 556 source lines and no unsafe boundary, while the Rust challenger measured 1,420 source lines and 21 `unsafe` mentions concentrated around Win32 interoperability. Rust required three direct Cargo dependencies / 14 resolved packages, a roughly 12.7-second clean optimized build, and a 438,272-byte stripped release executable. Node requires no compile step and no npm runtime packages, although the Node executable on the fixture was about 92.8 MB.

At the close of Spike 7, Rust was promoted only to **preferred candidate for deeper parity** while Node remained the working storage reference. Spike 8 subsequently completed that durability/dependency gate; D-153 supersedes this runtime-selection state.

Evidence: `spikes/phase1/results/2026-09-24-spike7-runtime-ipc-challenger-windows.json`.


## D-153 — Rust + bundled SQLite becomes the selected Phase 1 local core foundation

Status: Phase 1 selected

Spike 8 closes the runtime/storage selection gate that D-152 left open. The selected Phase 1 local foundation is now:

- Rust for the normal signed-in-user `relayd` host and canonical local CLI/runtime
- the kernel-verified current-user Windows named pipe plus random per-start token from Spike 7
- the embedded static/HTTP dashboard surface from D-151, hosted by the same Rust process
- SQLite for operational metadata/compact results/jobs/migration state
- `rusqlite 0.40.2` with default features disabled and bundled SQLite enabled
- schema version 1 kept database-compatible with the Node reference implementation

Node remains a compatibility/reference implementation during Phase 1, not the intended shipping daemon.

The neutral Spike 8 harness replayed the Spike 2 workload against both runtimes: 1 project, 300 result writes, 600 result reads, 100 checkpoint updates, and 20 `quick_check` calls. Both runtimes survived a hard process kill with the selected result, checkpoint, project registry, and schema intact. Both also started `Degraded` on a deliberately malformed database and on a future schema version, blocked storage writes, and preserved the damaged/future store instead of silently replacing or downgrading it.

Cross-runtime schema compatibility was tested in both directions: Node-created schema-1 projects/results/jobs were opened and read correctly by Rust, and Rust-created schema-1 data was opened and read correctly by Node. Stored payload hashes and producer versions were preserved. Node used SQLite 3.53.3; the Rust bundled build used SQLite 3.53.2.

The runtime advantage survived real storage:

- post-restart idle RSS: Node 120,500,224 bytes; Rust 9,367,552 bytes — Rust remained 92.23% lower
- initial storage-ready startup: Node 87.836 ms; Rust 46.915 ms
- restart storage-ready startup: Node 90.713 ms; Rust 47.205 ms
- result write p50: Node 1.031 ms; Rust 1.002 ms
- result read p50: Node 0.360 ms; Rust 0.343 ms
- `quick_check` p50: Node 0.382 ms; Rust 0.363 ms
- checkpoint update p50: Node 0.794 ms; Rust 1.563 ms
- both clean-close databases were 1,462,272 bytes and both cleared WAL/SHM files after clean shutdown

The Rust checkpoint path is slower on this fixture, but the absolute p50 remains low and does not reverse the runtime/resource decision. Concurrency and maintenance behavior remain separate gates rather than being inferred from single-writer latency.

The dependency/build cost also became concrete. Relative to the Spike 7 Rust host, the storage-enabled release executable grew from 438,272 bytes to 2,153,472 bytes, direct Cargo dependencies from 3 to 5, and the resolved package graph from 14 to 34 packages. A clean optimized build with cached crates measured 32.013 seconds; an incremental no-change release build measured 264 ms. Selected dependency metadata reports `rusqlite` MIT, `libsqlite3-sys` MIT, and `sha2` MIT OR Apache-2.0. The bundled SQLite 3.53.2 amalgamation contains SQLite's upstream copyright disclaimer/public-domain dedication; final installer notice generation and release-license auditing remain packaging gates.

This selection confirms D-147's SQLite operational-store architecture while superseding its Node-specific implementation path. It does not reopen D-148: heavyweight evidence remains outside SQLite by default.

Open storage gates remain concurrent-reader/writer behavior, long-reader WAL/checkpoint pressure, VACUUM/maintenance interruption, disk-full injection, backup/restore, and interrupted future migrations. Those are maintenance/release-hardening work; they no longer block the Phase 1 language/runtime choice.

D-153 supersedes the runtime-choice portions of D-146 and D-152.

Evidence: `spikes/phase1/results/2026-09-24-spike8-rust-storage-parity-windows.json`.

## D-154 — One JSON command registry is RELAY's Phase 1 semantic contract source

Status: Phase 1 selected

Spike 9 selects a plain machine-readable JSON command registry using the JSON Schema 2020-12 dialect with a deliberately bounded RELAY validation profile. The registry is the semantic source for command IDs, per-command contract versions, concise purpose, argument/result schemas, declared errors, effect/permission/idempotency classes, and surface visibility.

The selected Rust core embeds and validates the registry at startup. Unsupported registry-format versions, unsupported schema keywords, duplicate command/version pairs, and active reuse of reserved/deprecated command IDs fail closed. Arguments are validated before business logic; successful results and command error envelopes are validated before return. Deterministic validation requires no model call.

The current Phase 1 registry contains 13 commands. CLI catalog/help/describe metadata, dashboard command exposure, adapter discovery metadata, AI discovery metadata, and command capability IDs are all derived from the same registry. The dashboard no longer owns a separate command allowlist.

Command-version omission remains compatible with existing Spike 1–8 clients and currently resolves to the latest registered command version. A client may explicitly request a command contract version. Unsupported requested versions return `COMMAND_VERSION_INCOMPATIBLE`. Breaking command-shape changes require a new command contract version; additive optional fields may remain within a compatible version when the compatibility checker confirms older payloads remain accepted. Reserved/deprecated IDs may not be silently reused.

On the Windows fixture, registry validation added no direct Cargo dependency and no resolved package to the Spike 8 Rust graph: 5 direct dependencies / 34 resolved packages remained unchanged. The release executable grew 118,272 bytes, from 2,153,472 to 2,271,744 bytes. With the embedded dashboard open, sampled idle RSS was 8,523,776 bytes and no CPU time was observed in the five-second idle sample.

Measured p50 round trips were 0.275 ms for `system.status` with registry validation, 0.271 ms for compact AI `registry.list`, 0.220 ms for `registry.describe project.register`, and 0.189 ms for deterministic rejection of an invalid request.

The minified full registry was 10,408 bytes. Compact AI discovery was 1,971 bytes (18.94% of the full registry), while one `project.register` description was 855 bytes (8.21%). Using the transparent benchmark heuristic of four UTF-8 bytes per token, those are approximately 2,602, 493, and 214 tokens respectively; these are not model-specific tokenizer counts.

A general-purpose Rust `jsonschema` 0.57.0 cost probe with default features disabled resolved 80 packages, produced a 4,229,120-byte tiny validator executable, and took 72.5 seconds for the first optimized build on this fixture. RELAY therefore keeps its bounded validator instead of importing a general-purpose engine until real commands require unsupported JSON Schema features.

TypeSpec, CUE, and Protocol Buffers remain valid technologies, but Phase 1 does not add their compiler/language/code-generation toolchains to the local core. RELAY's current CLI, dashboard, gateway, adapter-discovery, and AI-facing contracts are already JSON-shaped, so a debuggable JSON registry minimizes translation and packaging cost. This choice does not prohibit generated bindings or alternate transports later.

The selected registry mechanism is not a freeze of the current 13-command catalog and is not a promise that every future extension contract must live in the built-in core registry. Signed/versioned extension contract packaging remains later work.

Evidence: `spikes/phase1/results/2026-09-24-spike9-command-registry-windows.json`.


## D-155 — Out-of-process adapter broker + manifest becomes the Phase 1 adapter execution foundation

Status: Phase 1 selected

Spike 10 selects the generic adapter execution foundation before any real UEFN or other tool adapter work.

The selected shape is:

- third-party adapter implementation code runs in a separate worker process, never loaded directly into RELAY Core
- manifest format 1 declares adapter identity/version, publisher/source, artifact digest, RELAY adapter-protocol range, command/version bindings, requested permissions, target-tool requirements, component/dependency metadata, update source/channel, build provenance, and review status
- the manifest requests capabilities; broker policy decides what is actually granted
- command bindings must resolve to the trusted D-154 command registry and adapter-visible command surface before a worker can launch
- the worker executable SHA-256 is verified before launch; the executable worker component carries its own version/source/digest inventory
- workers launch on demand rather than remaining resident merely because an adapter is installed
- worker identity, protocol, process ID, and exact capability set are rechecked during the worker hello
- arguments and results/errors are validated through the trusted RELAY command registry; adapter-provided schemas or prose do not become Core policy
- stdout, stderr, errors, and structured worker results remain untrusted adapter data with provenance attached
- crash/hang/invalid-message failures feed explicit restart backoff and quarantine state

Windows Job Objects are selected for the first containment layer. The broker sets and queries back:

- kill-on-job-close
- active-process limit of 1
- per-process memory limit

The synthetic fixture used a 32 MiB memory limit. A worker attempting to reserve 128 MiB reported failure. These exact resource values are spike settings, not final product defaults.

The neutral benchmark measured:

- manifest/digest/policy/command compatibility validation: 0.291 ms p50
- full on-demand worker invocation, including process launch, hello, Job Object assignment, registry validation, result validation, and teardown: 7.424 ms p50 / 9.103 ms p95
- synthetic worker crash surfaced as \`ADAPTER_WORKER_EXITED\` in 6.203 ms without crashing the broker
- a 100 ms hang test surfaced as \`ADAPTER_TIMEOUT\` in 122.097 ms
- invalid JSON surfaced as \`ADAPTER_INVALID_MESSAGE\`
- hostile stderr remained an untrusted observation and did not alter the broker's network-deny policy
- 0 installed adapters: 4,460,544 bytes RSS and 0 sampled CPU ms over five seconds
- 100 installed inactive adapters: 5,275,648 bytes RSS and 0 sampled CPU ms; the 100-adapter delta was 815,104 bytes
- both idle cases had zero adapter-worker processes; the observed child was Windows \`conhost.exe\`, not an adapter worker
- installing/validating 100 inactive manifests took 56.487 ms
- no new direct Cargo dependency or resolved package was added over Spike 9
- the selected core executable remained 2,271,744 bytes in this prototype because the broker is not yet wired into the daemon command surface; the synthetic worker executable was 215,040 bytes. Final daemon-integration binary cost remains to be measured.

Eight synthetic integration tests also covered over-permissioned manifests, incompatible protocol/command bindings, bad artifact/component digests, identity/capability mismatch, crash/backoff/quarantine, hang timeout, invalid JSON, bad result schema, undeclared worker errors, hostile stderr, and the memory cap.

This decision does **not** classify the current worker process as a security sandbox. The worker still executes under the signed-in user's token. Job Objects provide process lifecycle/resource containment but do not by themselves deny arbitrary filesystem, registry, local IPC, or network access. Manifest/policy denial therefore remains an authorization boundary enforced by the trusted broker, not an OS containment guarantee against a malicious worker.

Before open third-party adapters can be treated as strongly isolated, Phase 1 must test an OS-enforced Windows capability boundary for filesystem/network/process access while preserving the developer-tool integration workflows RELAY needs. Until then, third-party adapter execution remains experimental/controlled rather than a general marketplace security promise.

No UEFN, Fortnite, Blender, Krita, or other real adapter behavior was implemented in Spike 10.

Evidence: \`spikes/phase1/results/2026-09-24-spike10-adapter-broker-windows.json\`.


## D-156 — Strong adapter isolation requires an OS capability sandbox; the experimental Windows backend remains provisional

Status: Phase 1 selected security model; backend provisional

Spike 11 selects the security semantics that must wrap untrusted third-party adapter workers in addition to D-155's broker and Job Object containment.

The selected Windows boundary is:

- launch the worker in an AppContainer/process sandbox rather than under the unrestricted signed-in-user token
- grant only explicit filesystem paths; the broker-owned mailbox is read/write and the executable-worker directory is read-only
- deny filesystem access outside granted roots by default
- deny network access by default
- translate only broker-approved capability names into OS capabilities; unsupported capability names fail in RELAY before process launch
- require an explicit egress policy as well as the required Windows network capability before network is enabled
- supply a custom minimized environment instead of inheriting the parent environment
- inherit no handles
- disable Win32k system calls for the synthetic worker
- preserve the outer D-155 Job Object limits: one active process, process-memory limit, and kill-on-close
- fail closed when the sandbox contract/version is unsupported; never silently retry the worker unrestricted

On the measured Windows fixture, `processmodel.dll` exports Microsoft's experimental `Experimental_CreateProcessInSandbox` API and reports file version 10.0.26100.9444. The prototype dynamically loads this API rather than adding a hard loader dependency.

The synthetic worker itself verified `TokenIsAppContainer=true`. With no network grant, it could read/write the mailbox, read its executable from the read-only worker directory, but could not write that read-only directory, read or write a sibling blocked directory, open outbound TCP, observe the synthetic parent secret, observe `USERPROFILE`, or create a second process. Default TCP denial surfaced as WSAEACCES 10013. Child-process creation failed while the query-verified outer Job Object retained active-process limit 1, 32 MiB process-memory limit, and kill-on-close.

A positive egress test also passed. On this serviced build, the `internetClient` capability appeared in the AppContainer token but did not by itself permit TCP; the final explicit-grant path combines RELAY's capability allowlist, `internetClient`, and SandboxSpec egress default-allow. The same network-enabled worker still could not access the blocked filesystem roots or create a child process.

RELAY does not trust the experimental API to validate capability names. During exploratory probing on this fixture, Windows accepted an unknown capability string rather than rejecting launch as expected. The final path therefore allowlists supported capability names in the trusted broker before building the sandbox specification. An unsupported sandbox-spec version is also rejected before process launch, proving no unrestricted fallback.

Measured deny-mode sandbox cost across 30 fresh workers was:

- sandbox process creation: 30.037 ms p50 / 53.149 ms p95
- full sandboxed mailbox round trip: 63.200 ms p50 / 108.509 ms p95
- D-155 unsandboxed on-demand worker invocation reference: 7.424 ms p50
- strong sandbox overhead: about 55.776 ms p50, or 8.51× the unsandboxed synthetic worker path

The stronger boundary therefore has a real per-launch cost. Persistent worker pooling may later be benchmarked for trusted/high-frequency integrations, but no pooling exception may weaken the selected isolation semantics for untrusted adapters.

The implementation added one direct Rust dependency, `flatbuffers` 25.12.19 (Apache-2.0), and three resolved packages versus Spike 10. The release build graph grew from 34 to 37 packages. The selected core executable remained 2,271,744 bytes because the sandbox backend is not yet wired into the daemon command surface and unused code is stripped. The synthetic sandbox worker measured 256,000 bytes. A clean optimized all-binary build measured 36.074 seconds on cached crates.

The current direct backend is **not** selected as RELAY's permanent public Windows sandbox API. Microsoft documents `CreateProcessInSandbox` as experimental, and its behavior may change. D-156 selects the isolation policy/semantics and validates this backend on the measured host. Before public open third-party adapters, RELAY must prove a stable Windows process-container/fallback implementation and OS-version support matrix with the same adversarial fixture. If a host cannot provide the required strong boundary, untrusted adapter launch must remain unavailable rather than degrade to the D-155 worker-only model.

No UEFN, Fortnite, Blender, Krita, or other real adapter behavior was implemented in Spike 11.

Evidence: `spikes/phase1/results/2026-09-25-spike11-windows-adapter-sandbox.json`.


## D-157 — Stable LPAC AppContainer + brokered egress is the Windows release-candidate sandbox on qualified builds

Status: Phase 1 selected Windows sandbox backend; OS-build matrix conservative

Spike 12 replaces the experimental Spike 11 processmodel backend in RELAY's intended release path with Microsoft's documented desktop AppContainer launch path:

- `CreateAppContainerProfile` / AppContainer SID identity
- `SECURITY_CAPABILITIES` through `STARTUPINFOEX`
- `CreateProcessW` with extended startup attributes
- Low Privilege AppContainer behavior with All Application Packages opt-out
- Win32k system-call disable mitigation
- the existing D-155 outer Job Object limits

The strong isolation semantics from D-156 remain unchanged: filesystem outside grants is denied, direct worker network is denied, inherited environment is minimized, child-process creation remains bounded, and unsupported isolation never falls back to an unrestricted worker.

The stable release policy is deliberately narrower than the exploratory Spike 11 backend. An untrusted worker receives exactly one direct read/write path: a broker-owned ephemeral mailbox. Project/user writes must return through trusted broker commands. Worker/project paths may be read-only.

Network access is brokered rather than granted directly to the worker. The stable backend rejects direct capability requests before launch and exposes egress only through a trusted target allowlist.

On the final 30-run Windows fixture:

- stable LPAC prelaunch policy/ACL setup: 19.941 ms p50 / 25.328 ms p95
- stable `CreateProcessW` AppContainer launch: 5.517 ms p50 / 9.187 ms p95
- full stable sandbox round trip including temporary security restoration: 63.261 ms p50 / 86.259 ms p95
- experimental Spike 11 reference round trip: 65.297 ms p50 / 84.733 ms p95
- stable-vs-experimental p50 ratio: 0.9688; the documented stable path remained slightly faster at p50 on the final rerun

The worker verified `TokenIsAppContainer=true`, could use the mailbox, could not write the read-only worker tree, could not read/write blocked filesystem paths, could not use direct TCP, did not inherit the synthetic parent secret or `USERPROFILE`, and could not create a child process. The outer Job Object again query-verified one active process, 32 MiB process-memory limit, and kill-on-close.

Temporary filesystem security changes are bounded to the broker-owned mailbox and read-only worker tree. Integration tests and the benchmark verify that the DACL is restored, the temporary Low-Integrity label is removed after exit, and the worker-tree descriptor is restored exactly. The mailbox is ephemeral and deleted by the broker fixture.

The release selector uses matrix version 1 and enables strong untrusted launch only on Windows builds that have passed RELAY's full adversarial fixture. Build 26200 is the only physically qualified build in this spike. A simulated experimental-only host and a simulated unmeasured stable-API host both select `disabled`; a measured host still selects the stable backend even if the experimental API is absent.

Spike 12 added no direct Cargo dependency, no resolved package, and no selected core-binary growth over Spike 11. A clean optimized all-binary build measured 40.017 seconds on cached crates. The stable probe executable measured 390,144 bytes; the synthetic sandbox worker measured 248,832 bytes. The stable backend/source selector added 1,686 selected source lines and 66 `unsafe` mentions, concentrated at the Windows ABI/security boundary.

The experimental `Experimental_CreateProcessInSandbox` backend remains in the Phase 1 repository only as a measured reference. It is never a release fallback.

Microsoft documents the stable AppContainer APIs back to Windows 8 desktop apps, but documentation availability is not treated as RELAY qualification. Additional Windows builds must run the same adversarial suite before entering the release allowlist. Real UEFN/Blender/Krita adapter compatibility inside this boundary also remains unproven.

No UEFN, Fortnite, Blender, Krita, or other real adapter product behavior was implemented in Spike 12.

Evidence: `spikes/phase1/results/2026-09-25-spike12-stable-windows-sandbox-matrix.json`.

## D-158 — Signed side-by-side bundles are the default personal Windows install/update path

Status: Phase 1 selected packaging/update model; MSIX retained as optional managed channel

Spike 13 compares signed MSIX + App Installer against a RELAY-owned per-user side-by-side updater using the same synthetic Rust core + adapter payload.

The default personal/direct-download path is the signed side-by-side model:

- versioned binaries live under a per-user install root
- updates arrive as ZIP payloads with a detached CMS signature over the bundle SHA-256 digest
- the updater verifies signer identity, signed digest, release metadata, and every component hash before staging
- staging never changes the active version
- activation changes one fsync'd `current.json` pointer only after compatibility checks
- rollback activates a previously verified version only after storage-schema preflight
- uninstall removes binaries/version state but leaves separately owned durable RELAY/project data untouched
- no Session 0 service is required

On the non-elevated Windows fixture, the side-by-side path physically completed install, tamper rejection, staged update, activation, incompatible rollback rejection, compatible rollback, uninstall, and durable-data survival.
Measured side-by-side results:

- v2 ZIP payload: 1,317,397 bytes plus 1,239-byte CMS signature for a 2,486,784-byte raw two-component payload
- v2 build/sign/verify: 1.169 seconds
- v1 install + verification + activation: 412.206 ms
- v2 stage/verification without activation: 402.001 ms
- atomic v2 activation: 3.392 ms
- compatible rollback to v1: 4.493 ms
- uninstall binaries/version state: 4.305 ms
- the prototype updater is 245 source lines / 7,297 bytes

Tampering the ZIP caused `UPDATE_SIGNATURE_INVALID` before extraction and left v1 active. Staging valid v2 left v1 active until the explicit activation step. Simulated storage schema 2 caused v1 rollback to fail `UPDATE_STORAGE_SCHEMA_INCOMPATIBLE` while v2 remained active.

Both active-version records retained channel/source, maximum storage schema, and the core/adapter version + SHA-256 + provenance inventory. The external marker and SQLite fixture survived both update candidates' uninstall paths.
MSIX/App Installer remains an optional Windows-managed/Store channel rather than the default direct-download path. The fixture produced valid signed v1/v2 MSIX packages, rejected a tampered signature, verified both packaged component inventories, and generated an upgrade-only App Installer definition with on-launch/background checks.

The v2 MSIX measured 1,349,288 bytes, about 2.3% larger than the side-by-side ZIP + CMS signature. Build/sign/verify measured 1.264 seconds on the final fixture.

The non-admin fixture could not physically register the self-signed MSIX: AppX returned `0x80073CF0` with certificate trust error `0x800B0109`, even after temporary current-user trust. That is recorded as a distribution/trust dependency, not papered over with elevation. Public MSIX distribution requires a certificate Windows already trusts or Store/managed signing.

The Phase 1 Node side-by-side updater is evidence code, not the shipping updater. Production must implement the selected model in the shipping stack, define the publisher-key/trust bootstrap without installing a test root, use HTTPS for automated delivery, and keep rollback/data-schema checks separate.

No UEFN, Fortnite, Blender, Krita, or real adapter payload was packaged.

Evidence: `spikes/phase1/results/2026-09-25-spike13-packaging-update-windows.json`.


## D-159 — Bounded structured JSONL is the default local diagnostic record; ETW is optional deep tracing

Status: Phase 1 selected logging/diagnostic foundation

Spike 14 selects a bounded append-only JSONL diagnostic record for the normal per-user support path.

The selected foundation is:

- one versioned structured event envelope with stable event ID, timestamp, severity, component, project/job/result references, correlation/causation IDs, source/trust metadata, declared completeness/sampling state, capture mode, and structured attributes
- deterministic redaction of credential/secret/token/path keys plus path-looking string values before persistence
- normal capture with bounded strings/collections and a 4 KiB event ceiling
- explicit temporary detail mode, capped to 15 minutes and a 32 KiB event ceiling
- append-only current JSONL plus bounded rotated files; retention eviction is counted rather than presented as complete history
- periodic `sync_data` durability with a Phase 1 normal window of 16 events, plus explicit flush on shutdown/support boundaries
- partial-tail restart recovery that removes only the incomplete trailing record and reports the recovered byte count
- aggregate/health support summaries by default; unrestricted raw history and private event attributes are not included in the default support export
- diagnostic capture has its own health state. A logging failure marks live host recovery/doctor state Degraded rather than allowing otherwise healthy storage/IPC state to hide missing evidence
- normal command instrumentation records command identity/version/outcome/error code and correlation ID only; it does not persist command arguments or results

The neutral Windows benchmark used the same 10,000-event synthetic envelope for JSONL, SQLite, and ETW.

Default JSONL with a 16-event durability window measured:

- 3,685.9 events/second
- 0.0196 ms p50 append latency
- 3.8777 ms p50 durability-sync boundary latency
- 3,735,997 retained bytes after bounded rotation in the 4 x 1 MiB benchmark fixture
- 5,289 events / 4 files explicitly recorded as retention-evicted
- 19.707 ms aggregate-read cost
- 5,197,824 bytes idle RSS and 0 sampled CPU ms over five seconds

The sync-every-event control measured 4.1647 ms p50 and 239.4 events/second. This makes the durability/throughput trade explicit rather than implying every normal event is power-loss durable immediately.

SQLite was substantially faster at the same 16-event durability window:

- 24,350.6 events/second
- 0.0030 ms p50 insert latency
- 0.4272 ms p50 16-event commit latency
- 4,247,552 retained bytes after retaining 4,096 rows
- 108.608 ms combined prune/checkpoint/VACUUM/post-VACUUM-checkpoint maintenance in the measured run
- 6,242,304 bytes idle RSS and 0 sampled CPU ms

SQLite is therefore not rejected for performance. It remains selected for operational state under D-153. It is not selected as the default raw diagnostic record because JSONL already exceeds the expected diagnostic event rate by a wide margin while keeping raw support evidence directly inspectable/salvageable, bounded by simple file rotation, and outside the operational SQLite/WAL/checkpoint/VACUUM failure and maintenance domain.

ETW with no active consumer measured effectively zero write-call latency and 0 retained bytes; the provider was disabled and the path was non-durable. Attempting to start a durable ETW consumer session as the normal non-elevated user returned Access Denied. ETW therefore remains an optional Windows deep-tracing hook for explicit diagnostic sessions, not the default support record. If a later product path enables ETW sessions, provider/session loss counters must be surfaced as diagnostic completeness degradation.

Temporary detail mode increased bytes/event by 4.187x and p50 write latency by 1.106x in the synthetic fixture while remaining below the declared 32 KiB event ceiling. A deliberately partial JSONL tail recovered 11 bytes, preserved the prior complete event, and left zero invalid lines.

The candidate added no direct Rust dependency and no resolved Cargo package. The existing `windows-sys` dependency only enabled the ETW feature surface. The clean optimized core/probe build measured 37.447 seconds, and the release core measured 2,360,832 bytes in this final run. The benchmark/prototype diagnostics source contains the platform ETW unsafe calls; the normal JSONL storage implementation itself does not require a new native/unsafe boundary.

The Phase 1 module defaults (512 KiB current/rotated-file target, four retained files, 16-event sync window) are prototype defaults, not public retention policy. Hardware-tier tuning, final user-facing support-bundle archive format, encryption-at-rest policy, remote telemetry/analytics, and elevated ETW session benchmarking remain later work.

No UEFN, Fortnite, Blender, Krita, or real adapter behavior was implemented in Spike 14.

Evidence: `spikes/phase1/results/2026-09-25-spike14-diagnostics-foundation-windows.json`.
