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
