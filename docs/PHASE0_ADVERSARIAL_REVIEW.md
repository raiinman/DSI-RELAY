# Phase 0 Adversarial Review

## Status

Phase 0 is reopened.

This review deliberately attacks RELAY's current assumptions before implementation. The goal is not to prove the concept right; it is to find where the design is weak, overly confident, expensive, unsafe, duplicative, or likely to age badly.

## Method

Evidence priority:

1. Current first-party platform documentation.
2. Government standards, research, and technical reports.
3. Peer-reviewed or established academic research.
4. Recent preprints with clearly labeled confidence limits.
5. Community experience reports as hypothesis generators only.

A design idea is classified as:

- KEEP — evidence strengthens the current direction.
- MODIFY — keep the intent but change the contract.
- BENCHMARK — plausible, but do not make it a default until measured.
- DEMOTE — useful option, not a primary architectural rule.
- REJECT — evidence is strong enough to remove the idea.

See EVIDENCE_REGISTER.md for sources.

## Executive result

The RELAY concept survives the attack, but several parts need harder boundaries.

Strongest surviving ideas:

- provider-independent core
- local deterministic computation
- durable external project state instead of chat-as-database
- compact progressive AI output
- headless reproducible execution
- multi-project isolation
- dashboard plus CLI over one command system
- transactions, state verification, and result IDs
- version/capability-gated adapters

Ideas that require modification:

- "CLI + skills is better than MCP" becomes a benchmarked client strategy, not a universal truth.
- Context compression becomes provenance-, intent-, freshness-, and conflict-aware context compilation.
- "store all raw output" requires lifecycle, quota, privacy, and secret-handling policy.
- human approval becomes risk-adaptive plans/approvals to avoid consent fatigue.
- automatic self-repair needs rate limits, escalation, visible state, and strict scope.
- RELAY should integrate authoritative engine-native diagnostics instead of cloning them.
- prompt injection must be treated as a first-class boundary because project content itself can be hostile.

## Attack 1 — Bigger context may not solve context problems

### Evidence

Long-context models can fail to use relevant information reliably when it is buried in the middle of a large prompt. Recent agent research also reports semantic drift and context explosion in long software-engineering trajectories.

### Verdict

KEEP the Context Compiler.

### Required changes

The compiler must not optimize only for token reduction. It must track:

- provenance
- current goal/intent
- source freshness
- version
- trust level
- exact protected fields
- conflicts between old and current state
- retrieval reason
- evidence references

Context eviction must remain distinct from deletion.

## Attack 2 — Compression can quietly delete the one fact that matters

### Evidence

Prompt-compression research finds that some methods preserve overall semantics while losing key details. Recent structured-memory work explicitly protects atomic numerical/entity constraints.

### Verdict

MODIFY.

### Required changes

RELAY must separate:

- exact structured facts
- derived summaries
- raw evidence

Exact IDs, paths, coordinates, versions, measurements, errors, permissions, transactions, and result references must never depend on a lossy summary.

Compression benchmarks must score information preservation and grounding, not only task success and token ratio.

## Attack 3 — Memory can poison future decisions

### Evidence

2026 agent-memory work reports an experience-following effect: similar retrieved experiences can strongly shape future outputs, allowing past errors or mismatched examples to propagate. Intent-aware memory research also shows that semantically similar history can be wrong for the current goal.

### Verdict

MODIFY.

### Required changes

RELAY memory needs:

- quality/confidence metadata
- contextual intent
- source and timestamp
- current-vs-historical distinction
- stale-state invalidation
- conflict detection
- ability to quarantine bad derived memories
- retrieval audit explaining why an item was selected

Past successful execution is evidence, not an instruction to repeat the same action.

## Attack 4 — "CLI beats MCP" is too absolute

### Evidence

Research on MCP tool descriptions shows that poor descriptions are widespread and can hurt selection, but compact high-quality descriptions can improve reliability. Recent selective tool-discovery systems report very large reductions in tool-schema tokens by exposing only a small relevant subset.

The more general lesson from software-agent research is that the agent-computer interface matters. A bad CLI wrapper can be worse than a good tool surface.

### Verdict

BENCHMARK.

### Required changes

Keep:

- RELAY Core as the source of behavior
- structured headless execution as the stable machine contract
- CLI as the canonical local interface

Change:

- compact skills + CLI are the initial hypothesis for local AI clients, not a guaranteed winner
- thin MCP with dynamic capability/tool discovery remains a serious candidate
- client defaults must be selected by measured success/cost, not ideology

The benchmark must compare at least:

1. CLI + compact skill
2. thin MCP with search/execute/result
3. domain MCP with dynamically selected tools
4. broader static tool catalog

## Attack 5 — "Human approval makes it safe" can backfire

### Evidence

NIST's 2026 agent-identity work warns that overly chatty human-in-the-loop systems can cause consent fatigue, conditioning users to reflexively approve requests.

Older human-factors work reaches the same broad lesson from automation: overreliance, false alarms, poor feedback, and loss of situation awareness can make nominally supervised automation less safe.

### Verdict

MODIFY.

### Required changes

Approvals should be risk-adaptive:

- show a bounded "flight plan" or change plan where possible
- batch related actions
- require new approval when scope materially changes
- reserve frequent prompts for genuinely new risk
- show what authority the agent currently has
- show what changed after execution
- make pause/revoke obvious

Measure approval frequency, rejection rate, and repeated low-value prompts.

## Attack 6 — Automation can make the human worse at recovery

### Evidence

Decades of automation research describe the "ironies of automation," misuse/disuse, complacency, out-of-the-loop performance, and automation surprise. NASA studies found serious problems at the human-automation interface even when the automation itself was reliable.

### Verdict

MODIFY.

### Required changes

The dashboard must preserve operator situation awareness:

- current automation mode is always visible
- active authority/permissions are visible
- important background actions are traceable
- changes are explained in plain language
- recovery paths are rehearsable/testable
- healthy state should be quiet
- warnings need severity/confidence and deduplication
- false-positive-heavy detectors must not become permanent red banners

Self-repair must stop escalating silently after repeated failure.

## Attack 7 — Project content is an attack surface

### Evidence

NIST/CAISI's 2026 large-scale agent red-team studied more than 250,000 attacks from over 400 participants across 13 frontier models and found at least one successful hijacking attack against every target model. The threat includes malicious instructions embedded in emails, websites, and code repositories.

RELAY will ingest exactly this class of external data: project files, docs, logs, generated content, downloaded assets, repositories, and tool output.

### Verdict

NEW HARD REQUIREMENT.

### Required changes

All project/tool content is untrusted data by default.

The Context Compiler must:

- label provenance and trust
- separate instructions/policy from retrieved project data
- avoid promoting retrieved text into higher-authority instructions
- sanitize/render potentially hostile content safely
- constrain actions through structured commands and permissions
- preserve an audit trail from evidence to action

Model-only prompt defenses are insufficient.

## Attack 8 — Local agents using the user's account are too powerful

### Evidence

NIST's 2026 agent identity guidance warns that local agents running with the user's credentials can impersonate the user and inherit broad access. NIST recommends first-class agent identities, scoped authorization, short-lived credentials, and hardened/sandboxed execution.

### Verdict

MODIFY SECURITY MODEL.

### Required changes

Where integrations permit it:

- agent/client identity is separate from user identity
- delegated rights are scoped by project/action
- credentials are short-lived or revocable
- standing privilege is minimized
- local workers are sandboxed where practical
- remote commands never become arbitrary shell execution
- audit logs identify the requesting agent/client and the delegating user

## Attack 9 — Keeping every raw result forever is not free

### Problem

Our "never throw away evidence" idea improves recoverability but can create unlimited disk use, privacy exposure, secret retention, screenshot accumulation, and slow indexes.

### Verdict

MODIFY.

### Required changes

Define evidence classes with:

- retention period
- project quota
- pin/keep capability
- content hash/deduplication
- sensitive-data classification
- deletion/export behavior
- migration behavior
- backup expectations
- secure cleanup rules

"Context eviction is not deletion" remains true. "Nothing is ever deleted" does not.

## Attack 10 — Cost savings can be fake if we measure the wrong counterfactual

### Problem

"Tokens avoided" is easy to exaggerate because the hypothetical alternative may never have been run.

NIST's benchmark guidance emphasizes defining the measurement target, choosing fitting benchmarks and baselines, and reporting enough protocol detail for valid interpretation and reproducibility.

### Verdict

MODIFY.

### Required changes

Usage reporting must separate:

- directly measured values
- derived values
- estimated counterfactual savings

Claims such as "90% cheaper" require matched benchmark runs or clearly stated assumptions.

Measure local CPU, memory, storage, network, elapsed time, AI calls, remote calls, and operator time where practical—not tokens alone.

## Attack 11 — UEFN MCP is useful but volatile

### Evidence

Epic describes Unreal MCP in UE 5.8 as Experimental and explicitly says features are incomplete or missing and APIs/data formats may change. UEFN added Unreal MCP in Fortnite 42.00.

### Verdict

KEEP ADAPTER, REJECT DEPENDENCY.

### Required changes

- capability/version detection is mandatory
- adapter contracts shield RELAY Core from UEFN MCP changes
- unsupported capability returns a clear unavailable state
- avoid storing Epic-specific schemas in core
- maintain fallback paths where technically possible
- integration tests should target supported version ranges

## Attack 12 — RELAY must not duplicate the engine

### Problem

UEFN/Unreal already provides validation, session inspection, profiling, memory tools, transaction mechanisms, logs, debug drawing, and growing MCP/toolset capabilities. Reimplementing authoritative engine diagnostics creates stale rules and maintenance cost.

### Verdict

MODIFY.

### Required changes

Adopt native-tool-first integration:

1. use authoritative engine validation/measurement when available
2. normalize it into RELAY results
3. add RELAY-specific cross-tool/project checks only where the engine lacks them
4. label the source of each finding

RELAY should be the orchestrator, historian, context manager, and cross-tool observability layer—not a shadow Unreal Engine.

## Attack 13 — Benchmarks can lie too

### Evidence

NIST AI 800-2 says consistent benchmark practices are still emerging and emphasizes explicit objectives, relevant baselines, protocol reporting, uncertainty, and reproducibility.

### Verdict

MODIFY RESEARCH PROCESS.

### Required changes

Every RELAY benchmark must define before running:

- question/decision it informs
- measurement construct
- workload/tasks
- model/client versions
- tool/interface versions
- baseline
- repetitions/seeds where applicable
- pass/fail metric
- cost controls
- hardware/runtime where relevant
- uncertainty or run-to-run variation
- limitations/external validity

Do not optimize RELAY to a single benchmark suite.

## What remains strong

After adversarial review, these are still strong architectural contracts:

- provider independence
- one core command system
- reproducible headless execution
- multi-project isolation
- deterministic local work before AI work
- compact/progressive AI output
- durable result IDs
- post-write verification
- transaction history
- public-ready configuration
- adapter boundaries
- dashboard for human observability
- explicit cost instrumentation

## Phase 0 reopening gate

Phase 0 may close again only when:

1. this adversarial review is reflected in the owning architecture docs
2. affected decisions are reclassified in DECISION_LOG.md
3. security includes prompt-injection and agent-identity boundaries
4. context design includes provenance, intent, freshness, conflict handling, and memory-quality controls
5. automation design addresses consent/alarm fatigue and out-of-the-loop risk
6. evidence-retention lifecycle requirements are documented
7. UEFN design is native-tool-first and treats MCP as versioned/experimental
8. benchmark methodology follows an explicit measurement protocol
9. Phase 1 is prohibited from locking a stack before these requirements are testable


## Attack 14 — Tool descriptions affect agent behavior

### Evidence

Peer-reviewed 2025 work on agentic tool preferences found that small edits to tool descriptions could materially change which tools models selected. Separate 2026 research on tool shortlists also reports that the number of visible tools affects downstream selection quality.

### Verdict

MODIFY COMMAND-DESCRIPTION DESIGN.

### Required changes

- keep canonical command semantics, arguments, permissions, and effects centralized
- allow surface-specific rendering for CLI help, skills, dashboard, MCP, and remote clients
- test description wording for selection reliability and ambiguity
- expose only the smallest relevant command/tool shortlist when practical
- treat description changes as behavior-affecting changes that deserve tests and versioning

## Attack 15 — Compression itself can become an expensive loop

### Evidence

ACON reports meaningful reductions in peak tokens, but also introduces a compressor and explores distilling that compressor into smaller models to reduce overhead. A 2026 Focus preprint reports savings on only five SWE-bench Lite tasks, which is too small a sample to justify a universal default. Other 2026 work reports that implicit context compression can fail on multi-step coding workflows even when it performs well on simpler tasks.

### Verdict

MODIFY.

### Required changes

The Context Compiler should use a cost ladder:

1. deterministic filtering and deduplication
2. exact structured projection
3. cached summaries
4. cheap/local compression where measured useful
5. larger-model summarization only when the value exceeds the added cost

Every compression strategy needs a break-even benchmark that includes the cost of compression itself.

## Attack 16 — Permission categories are too one-dimensional

### Evidence

NIST's 2025 agent-tool taxonomy recommends reasoning about tool use across multiple dimensions, including function, read/write access, reversibility, reliability, monitoring, and autonomy.

### Verdict

MODIFY COMMAND METADATA.

### Required changes

Each side-effecting command should be able to declare, where applicable:

- read / constrained-write / write
- trusted versus untrusted target environment
- reversibility
- persistence or externality of effects
- required monitoring/verification
- autonomy/approval class
- credential scope
- affected project/resource boundary

Approval policy should derive from these attributes rather than only from a flat category.

## Attack 17 — Benchmarks can overstate real capability

### Evidence

NIST/CAISI has documented methodological problems in agent evaluations, including agents exploiting properties of evaluation environments in ways that can inflate apparent performance. Government evaluation guidance also emphasizes clear constructs, baselines, protocol disclosure, and field validation.

### Verdict

MODIFY BENCHMARK GOVERNANCE.

### Required changes

The RELAY benchmark program needs:

- held-out tasks not used to tune prompts/descriptions
- negative and stress cases
- version-pinned evaluation environments
- separation between tuning artifacts and evaluation tasks
- review of agent traces, not only final pass/fail
- periodic real-project dogfood evaluation outside benchmark fixtures
- explicit separation between benchmark score and production claim

## Attack 18 — Model behavior is not an authorization boundary

### Evidence

NIST/CAISI's March 2026 analysis of more than 250,000 red-team attempts across 13 frontier models found at least one successful hijacking attempt against every target model. NIST's 2026 agent-security RFI analysis also reports broad agreement that agent security needs adaptations beyond traditional controls.

### Verdict

KEEP AND STRENGTHEN THE HARD BOUNDARY.

### Required changes

- the model may propose an action
- RELAY policy decides whether the action is permitted
- untrusted content cannot grant authority
- retrieved text cannot alter approval scope
- execution uses validated structured commands
- important actions remain attributable and auditable
- prompt-injection testing becomes a release-gate class

## Attack 19 — Successful autonomous coding systems do not prove a general agent architecture

### Evidence

DARPA's AI Cyber Challenge demonstrated autonomous systems that found and patched vulnerabilities in open-source software under a tightly specified competition framework. This supports bounded specialist automation, not unconstrained agent authority.

### Verdict

KEEP SPECIALIZATION; REJECT OVERGENERALIZATION.

### Required changes

RELAY should favor bounded workflows with explicit inputs, outputs, verification, and permissions. General model reasoning may orchestrate those workflows, but broad authority must not be inferred from coding capability.

## Phase 0 additional closure requirements

Phase 0 now also requires:

10. command/tool descriptions and shortlist policies are treated as behavior and included in interface benchmarks
11. Context Compiler cost accounting includes the cost of compression itself
12. command metadata supports multidimensional risk/authority attributes
13. benchmark design includes held-out tasks, anti-gaming checks, and trace review
14. authorization is enforced by RELAY policy boundaries rather than model behavior


## Attack 20 — Local inference is not automatically cheaper

### Evidence

Recent systems research does not support a universal rule that local model inference is cheaper, faster, or more energy-efficient.

- A 2026 mobile-device study reported lower energy efficiency for on-device inference than batched server inference on its tested devices.
- A 2025 mobile/edge/cloud measurement study found only smaller models ran comfortably on phones and cloud inference was faster for its workload.
- Other 2025–2026 work finds local consumer GPUs or hybrid edge/cloud routing can reduce cost under different workloads.

These studies use different hardware and workloads, so RELAY must measure its own desktop workloads.

### Verdict

MODIFY THE COST PRINCIPLE.

### Required changes

Keep local deterministic work first.

Treat model placement as a measured routing decision based on:

- task capability requirements
- privacy
- latency
- utilization
- hardware
- energy/thermal conditions
- actual API and local operating cost

Do not claim local-model savings without matched measurements.

## Attack 21 — Incremental file watchers are not a source of truth

### Evidence

Microsoft documents that ReadDirectoryChangesW can overflow its buffer, discard buffered change details, and require directory re-enumeration. .NET FileSystemWatcher likewise documents that overflow causes file-system events to be lost.

NTFS change journals are more durable, but Microsoft also documents that old records can be deleted and journal discontinuities can occur.

### Verdict

MODIFY INDEXING ARCHITECTURE.

### Required changes

- filesystem watchers are accelerators, not truth
- detect watcher overflow and monitoring errors
- reconcile stored state against the actual project at startup
- support periodic or triggered reconciliation
- use NTFS journal information only as an optimization, with reconciliation after discontinuity
- persist checkpoints only after durable index updates
- make indexing idempotent and safe to replay

## Attack 22 — Derived state can silently diverge from the project

### Evidence

SQLite documents that WAL mode is unsuitable for network filesystems, that WAL files are part of persistent database state, and that live databases require supported backup techniques. SQLite also provides explicit integrity checks and online backup mechanisms.

The larger risk is independent of database choice: a derived index can become stale or damaged while appearing authoritative.

### Verdict

MODIFY STORAGE CONTRACT.

### Required changes

- project files and authoritative engine/runtime state remain the source of truth
- RELAY indexes and normalized state are derived and rebuildable
- mutable RELAY databases should default to a local application-data location rather than a project directory that may be synced, network-mounted, or version-controlled
- portable project configuration may remain with a project when useful
- storage must support integrity checks, migrations, backups/snapshots, and recovery
- Phase 1 storage tests must include abrupt termination and recovery
- stale or uncertain state must be reconciled before being presented as high-confidence current state


## Attack 23 — A single traditional Windows service conflicts with our first integrations

### Evidence

Microsoft documents that Windows services run outside the signed-in user's interactive desktop session on modern Windows. RELAY's first integrations—UEFN, Fortnite, Blender, and Krita—run in the user's interactive session.

Microsoft also provides service-account and service-isolation mechanisms specifically to reduce standing privilege.

### Verdict

REJECT A SINGLE HIGH-PRIVILEGE SYSTEM SERVICE AS THE DEFAULT PROCESS MODEL.

### Required changes

Phase 1 must compare split local-runtime designs:

- a per-user RELAY host running in the signed-in user's security context for project access and interactive tool adapters
- an optional minimal helper only for operations that genuinely require elevated operating-system rights
- isolated worker processes for riskier parsing/build tasks where practical
- no requirement for the main RELAY host to run as administrator or LocalSystem
- treat "relayd" as a logical background component name, not a commitment to Session-0 Windows-service architecture

Candidate implementations include a normal per-user background process and Windows per-user service mechanisms.

## Attack 24 — Local IPC needs explicit identity and access rules

### Evidence

Microsoft's named-pipe documentation shows that pipe access is controlled through Windows security descriptors and access checks; relying on defaults can grant broader access than intended.

### Verdict

NEW LOCAL-IPC REQUIREMENT.

### Required changes

- define explicit access control for local IPC
- do not treat loopback network location as user identity
- distinguish dashboard/client identity from worker/host identity
- require protocol/version negotiation
- fail closed on incompatible or unauthorized peers
- test cross-user and lower-privilege access behavior
- settle local IPC security before building the remote-gateway layer

## Attack 25 — Public auto-update is part of the trusted computing base

### Evidence

NIST's Secure Software Development Framework treats secure software delivery and lifecycle practices as part of normal development. CISA/FBI Secure-by-Design guidance similarly emphasizes reducing security risk at the software-manufacturer level. Supply-chain frameworks such as SLSA emphasize build provenance and artifact integrity.

### Verdict

MODIFY UPDATE DESIGN.

### Required changes

Before automatic updates are enabled for public RELAY installations:

- verify release artifact authenticity/integrity
- protect release-channel metadata
- retain build provenance where practical
- design update rollback/recovery
- keep elevated installer/update functionality narrowly scoped
- ensure adapters/plugins cannot bypass update-integrity policy
- ensure a failed update cannot destroy project data


## Attack 26 — A third-party adapter is executable supply-chain code

### Evidence

Developer-extension ecosystems show that third-party integrations can become a direct compromise path. A study of 52,880 VS Code extensions found about 5.6% exhibited suspicious behavior, and MCP-ecosystem studies have demonstrated that community servers and registries can expose users to tool poisoning, server takeover, and unsafe local actions.

### Verdict

REJECT IN-PROCESS UNTRUSTED ADAPTERS AS THE DEFAULT PUBLIC EXTENSION MODEL.

### Required changes

- third-party adapters should run out of process by default
- adapters must not receive direct access to RELAY Core internals or its database
- adapter communication must pass through a brokered, versioned protocol
- adapter crashes or hangs must not bring down RELAY Core
- per-adapter resource and timeout limits are required
- first-party adapters should use the same manifest/capability model where practical so the public extension path is continuously exercised

## Attack 27 — Marketplace review cannot be our trust boundary

### Evidence

A 2025 MCP study uploaded unsafe servers to multiple aggregation platforms and found existing review mechanisms insufficient. A 2025–2026 study of 67,057 MCP servers found weak registry vetting and server-hijack risks. Similar problems have been documented in browser-extension and developer-extension marketplaces.

### Verdict

DEMOTE AN OPEN RELAY MARKETPLACE.

### Required changes

- do not make an open public marketplace a v0.x requirement
- initial public extension support should prefer explicit local installation or curated sources
- publisher identity, version, digest, permissions, provenance, and update source must be visible before installation
- registry presence or a "verified" badge must never be treated as proof of safety
- unverified adapters must be clearly separated from first-party or reviewed adapters

## Attack 28 — Signatures prove origin/integrity, not good behavior

### Evidence

Software-supply-chain guidance from NIST emphasizes provenance, component inventories, artifact integrity, and secure development practices. Real package compromises also show that a previously trusted publisher or update channel can still distribute harmful code.

### Verdict

MODIFY TRUST SEMANTICS.

### Required changes

RELAY must distinguish:

- identity: who published it
- integrity: whether the artifact changed
- provenance: how/where it was built
- review status: what RELAY/project maintainers examined
- permissions: what it is allowed to do
- observed behavior: what it actually did at runtime

A valid signature must never imply "safe."

## Attack 29 — Adapter permissions need a capability contract

### Problem

An adapter that can read every project, execute arbitrary programs, use network access, access credentials, and emit unrestricted tool metadata effectively recreates a general-purpose remote-code execution surface.

### Verdict

NEW HARD REQUIREMENT.

### Required changes

Every adapter needs a declared capability manifest that can express, where relevant:

- project roots/resources it may read
- resources it may modify
- external applications/binaries it may invoke
- network access requirements
- secret/credential scopes
- subprocess needs
- editor/runtime endpoints
- whether writes are reversible
- AI/tool capabilities it exposes

RELAY policy grants only the approved subset. The adapter cannot expand its own authority.

## Attack 30 — Adapter dependencies and updates are part of the attack surface

### Evidence

The 2025 Nx/npm compromise is a concrete example of a trusted development package/update path being abused. NIST SSDF/SBOM guidance emphasizes component inventory, provenance, and update integrity.

### Verdict

MODIFY DISTRIBUTION AND UPDATE DESIGN.

### Required changes

- adapter versions are immutable and content-addressable where practical
- installs/updates are pinned to an exact artifact, not an unqualified "latest"
- transitive dependencies must be inventoried
- adapters should publish machine-readable component/dependency metadata
- update policy needs staging, health checks, and rollback
- dependency/update changes must be visible in the transaction/history system
- automatic adapter updates require the same integrity guarantees as RELAY updates

## Attack 31 — Adapter metadata can become an AI instruction channel

### Evidence

MCP research identifies tool-metadata poisoning as a practical attack vector. RELAY already treats project/tool content as untrusted; extension-provided names, descriptions, errors, documentation, and results belong in the same category.

### Verdict

EXTEND THE INSTRUCTION/DATA BOUNDARY.

### Required changes

- adapter-provided prose is untrusted data by default
- adapters cannot inject system/policy instructions into AI contexts
- command/tool semantics come from RELAY's registry and reviewed adapter manifests
- dynamic adapter output retains provenance/trust labels
- tool descriptions exposed to AI must be normalized/rendered by RELAY rather than blindly forwarded
- suspicious or changed tool metadata should be auditable by version/digest

## Attack 32 — Extensions can silently destroy the cost model

### Problem

A public ecosystem may add hundreds of commands, schemas, descriptions, health checks, background processes, and telemetry streams. Even safe extensions could recreate the context bloat RELAY is designed to remove.

### Verdict

MODIFY EXTENSION UX.

### Required changes

- adapter installation does not mean all adapter commands are exposed to every AI
- capability discovery remains task/project specific
- inactive adapters should consume near-zero AI context
- health polling and background work must be rate-limited and measurable
- per-adapter CPU/memory/storage/network and AI-context contribution should be observable
- extension benchmarks include context and runtime overhead, not only functionality

## Attack 33 — Extension compatibility needs quarantine, not optimism

### Problem

RELAY Core, external applications, and third-party adapters will evolve at different speeds. Loading an incompatible adapter into a privileged developer workflow can corrupt state or create misleading results.

### Verdict

NEW COMPATIBILITY REQUIREMENT.

### Required changes

- adapters declare RELAY API/protocol compatibility
- integrations declare supported external-tool versions/capabilities
- incompatible adapters fail closed and are quarantined/disabled
- migrations are explicit and versioned
- older adapter results retain the version/provenance needed for interpretation
- public release requires compatibility tests for first-party adapters and an SDK conformance suite for third parties

## Phase 0 adapter-ecosystem closure requirements

Phase 0 now also requires:

15. public extension architecture is out-of-process by default
16. adapter capability/permission manifests are defined conceptually
17. trust labels separate identity, integrity, provenance, review, permission, and runtime behavior
18. extension distribution/update policy includes pinned artifacts, dependency inventory, integrity checks, and rollback
19. extension-provided text is covered by the untrusted-content boundary
20. adapter cost/resource/context overhead is measurable
21. adapter API compatibility and quarantine behavior are defined before a public extension SDK is promised
