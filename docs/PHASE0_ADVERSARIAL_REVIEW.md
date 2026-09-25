# Phase 0 Adversarial Review

## Status

Closed — Phase 0 architecture and adversarial research baseline accepted on 2026-09-24.

Unresolved implementation choices and empirical validation move to Phase 1. Reopen Phase 0 only if new evidence materially invalidates the baseline rather than for normal implementation discoveries.

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


## Attack 34 — Out-of-process is fault isolation, not automatically a security sandbox

### Evidence

Windows provides explicit sandboxing mechanisms such as AppContainer/Win32 app isolation that restrict filesystem, network, credential, process, and other resource access. A normal child process running under the user's ordinary token does not gain those restrictions merely because it is a separate process.

### Verdict

CLARIFY THE ADAPTER-ISOLATION CONTRACT.

### Required changes

- out-of-process execution is the minimum boundary for crash/fault containment
- security isolation requires an OS-enforced restricted execution model or equivalent brokered capability design
- Phase 1 must benchmark Windows isolation options against adapter compatibility
- RELAY documentation must not use "out of process" and "sandboxed" as synonyms
- if an adapter cannot run in a restrictive sandbox, the dashboard must expose the resulting trust/permission level clearly

## Attack 35 — In-tool companion components extend the supply chain

### Problem

Some integrations may require code or scripts inside the target application as well as a RELAY-side adapter worker. Isolating the RELAY worker does not contain code that executes inside UEFN, Blender, Krita, or another host application.

### Verdict

EXTEND THE COMPONENT MODEL.

### Required changes

An adapter installation record must be able to enumerate all executable companion components:

- RELAY-side worker
- target-application plugin/extension/script
- bundled helper binaries
- runtime instrumentation package
- transitive libraries

Each component needs its own version/digest/provenance and compatibility status where practical. Updating one companion component must not silently change the trust state of the whole chain.


## Attack 36 — Cloud AI is a data-egress boundary, not just another compute target

### Evidence

NSA/CISA/FBI and partner guidance on AI data security treats data used throughout development, testing, and operation as part of the AI supply chain and emphasizes provenance, trusted infrastructure, and lifecycle protection. NIST's Privacy Framework likewise treats data processing and disclosure as explicit risk-management concerns.

### Verdict

NEW HARD DATA-PLANE REQUIREMENT.

### Required changes

RELAY must treat every remote model, embedding service, gateway, support upload, and networked third-party processor as an explicit egress destination.

Before remote processing:

- project policy determines whether the destination may receive the data
- the Context Compiler works only within the allowed data set
- the minimum sufficient subset is selected
- the outbound event is attributable and auditable
- unknown/stale provider data-use or retention properties remain unknown rather than being guessed

Remote AI is not allowed to become the implicit default path for private project material.

## Attack 37 — Secret redaction is not a credential architecture

### Evidence

OWASP guidance on system-prompt leakage explicitly warns against placing credentials and connection strings in model prompts. More generally, sensitive-information disclosure remains a known LLM-application failure class.

### Verdict

REJECT RAW CREDENTIALS IN MODEL CONTEXT.

### Required changes

- models receive opaque connection/credential handles, not secret values
- a trusted credential broker resolves secret material only at the execution boundary that needs it
- secrets do not enter prompts, embeddings, summaries, ordinary logs, or AI memory
- subprocesses receive only the specific credential required for that operation
- secret rotation/revocation does not require rewriting model-visible state
- secret-scanning/redaction remains defense-in-depth, not the primary control

## Attack 38 — Sensitive derived data can still reveal the source

### Evidence

Text-embedding inversion research has repeatedly shown that private source text can be reconstructed from embeddings. EMNLP 2023 reported exact recovery for much of short text under its tested conditions, ACL 2024 demonstrated transferable inversion without the original embedding model, and ACL 2025 ALGEN substantially reduced the amount of data needed for successful attacks.

### Verdict

REJECT "VECTOR = SAFE" ASSUMPTIONS.

### Required changes

- embeddings inherit source sensitivity
- remote embedding generation is a remote data-processing event
- vector stores require the same project isolation and lifecycle policy as other sensitive derived data
- source deletion/retention policy must address derived embeddings and indexes
- summaries, cached outputs, extracted metadata, and embeddings all retain provenance and sensitivity unless a defined declassification rule says otherwise

## Attack 39 — Screenshots are both sensitive data and a prompt-injection surface

### Evidence

Multimodal prompt-injection research shows that images can carry adversarial instructions that influence agents. OWASP also identifies multimodal injection as a risk. Separately, screenshots routinely capture source code, account identifiers, filenames, chats, unreleased assets, and occasionally credentials.

### Verdict

MODIFY VISUAL-OBSERVABILITY DESIGN.

### Required changes

- screenshots/images follow the same data classification and egress policy as text
- capture intended windows/regions rather than full desktops where possible
- image-only content is not assumed safe because text secret scanning found nothing
- visual content remains untrusted for instruction purposes
- remote visual analysis needs explicit project/provider policy
- local crop/redaction may reduce exposure but must not be represented as perfect sanitization

## Attack 40 — Prompt injection plus broad read access creates a data-exfiltration path

### Evidence

NIST/CAISI added database-exfiltration tasks to agent-hijacking evaluations and reported that agents could frequently be induced to follow malicious instructions. A 2025 study using AgentDojo found prompt-injection attacks could cause tool-calling agents to leak personal data observed during task execution, with no built-in defense fully preventing leakage in the extended evaluation.

### Verdict

SEPARATE DATA ACCESS FROM EGRESS AUTHORITY.

### Required changes

- permission to read project data does not imply permission to transmit it
- network/remote-model egress is separately authorized
- egress scope is bounded to the current task/project
- untrusted content cannot broaden retrieval or egress scope
- the model may request more context, but RELAY policy decides what can be disclosed
- high-sensitivity data and external-send capabilities should not be co-granted by default

## Attack 41 — Data minimization can fail if it is only a token optimization

### Evidence

NIST's Privacy Framework includes outcomes around selective collection/disclosure, data minimization, processing permissions, deletion, and limiting observability/linkability. These are privacy controls, not merely cost controls.

### Verdict

MODIFY CONTEXT-COMPILER ORDERING.

### Required changes

Context compilation must apply data policy before relevance/ranking:

1. determine destination and task
2. establish eligible data classes
3. exclude prohibited material
4. select the minimum sufficient relevant evidence
5. apply token/context-budget optimization
6. record lineage and policy decision

A smaller prompt is not necessarily a safer prompt if it still contains the wrong data.

## Attack 42 — "Local-only" is meaningless unless the whole product obeys it

### Problem

Even if model inference is local, project content can still leave through remote embeddings, crash reports, analytics, support bundles, remote gateways, adapter networking, update diagnostics, or future convenience features.

### Verdict

NEW PRIVACY MODE REQUIREMENT.

### Required changes

A local-only/private project mode must have testable egress semantics.

When enabled, project content should not leave through:

- remote model calls
- remote embedding services
- remote gateway payloads
- analytics containing project content
- automatic diagnostic upload
- third-party adapter network access unless separately authorized

Product update checks and other non-project networking must be documented separately so "local-only" is not a misleading label.

## Attack 43 — Provider privacy promises are versioned external dependencies

### Problem

Remote AI providers can differ by product tier, endpoint, workspace, region, retention, training/data-use policy, and time. RELAY cannot safely encode a single permanent rule such as "Provider X never trains on this."

### Verdict

NEW PROVIDER-POLICY PROFILE.

### Required changes

Each remote processor profile should record what RELAY actually knows:

- endpoint/account identity
- allowed data classes/modalities
- data-use/training policy source
- retention policy source
- region/residency where relevant
- date/version of policy verification
- organization/user overrides

Unknown or stale fields stay unknown and can trigger conservative policy.

## Attack 44 — AI responses can re-export sensitive input

### Problem

A response may quote, transform, summarize, or reproduce protected source material. Sending that response to another model, logging system, support bundle, or public share can create a second disclosure path.

### Verdict

EXTEND SENSITIVITY TO OUTPUTS.

### Required changes

- AI output inherits relevant source sensitivity/provenance
- model-to-model forwarding is a new egress decision
- result retention follows project data policy
- diagnostics and support exports do not automatically include raw model transcripts
- public/share/export actions pass through data policy again
- local deletion must not be represented as proof that a remote processor erased every copy

## Phase 0 data-boundary closure requirements

Phase 0 now also requires:

22. a documented data-classification and sensitivity-propagation model
23. a hard egress-policy boundary outside model reasoning
24. credential-handle/broker architecture that keeps raw secrets out of AI context
25. embeddings and derived artifacts inherit source sensitivity
26. screenshot/multimodal handling is included in privacy and prompt-injection policy
27. local-only/private mode has testable network-egress semantics
28. provider data-use/retention metadata is treated as versioned external policy, not assumption
29. outbound processing and sensitive result lineage are auditable


## Attack 45 — Automatic remote context collection can defeat RELAY's privacy model

### Evidence

Work on code assistants notes that cloud-based assistants may receive proprietary code as context, and recent research on AI coding assistants shows automatically gathered context can itself become an attack or leakage surface.

### Verdict

CONSTRAIN REMOTE CLIENT DATA ACCESS.

### Required changes

- connecting a remote AI client does not grant it raw project filesystem access
- RELAY remains the retrieval/egress mediator for project data
- context selection is local and policy-aware
- raw files are sent only when the task actually requires them and policy permits it
- provider/client convenience features that bypass RELAY's data boundary must be clearly out of scope or explicitly disabled where controllable

## Attack 46 — Metadata can be sensitive too

### Evidence

ACL 2025 research on knowledge-file leakage identified multiple leakage vectors involving not only file contents but metadata such as titles, types, and sizes, along with retrieval and execution pathways.

### Verdict

EXTEND DATA CLASSIFICATION BEYOND FILE CONTENT.

### Required changes

Potentially sensitive metadata includes:

- filenames and paths
- project/repository names
- asset names
- document titles/types/sizes
- account/user identifiers
- branch names
- integration names
- timestamps
- hashes where they reveal known content relationships

Egress policy applies to metadata when the project's sensitivity requires it.

## Attack 47 — Secret/privacy scanners cannot certify a payload as safe

### Problem

Pattern scanners can find known credential formats and privacy detectors can identify many structured entities, but proprietary code, unreleased designs, project strategy, and unknown secret formats may not match a detector.

### Verdict

KEEP SCANNERS AS DEFENSE-IN-DEPTH ONLY.

### Required changes

- allow explicit project/path/artifact sensitivity labels
- project defaults can be stricter than scanner output
- a "no findings" scan result must not be displayed as "safe to upload"
- deterministic redaction is useful but does not declassify a payload by itself
- user/org policy and provenance remain authoritative inputs to data classification


## Attack 48 — "The user approved it" is not a sufficient identity model

### Evidence

NIST's 2026 software-agent identity work explicitly calls out identification, authorization, auditing, and non-repudiation for agents. OAuth token-exchange standards likewise distinguish delegation from impersonation and can preserve both the subject and the acting party.

### Verdict

MODIFY THE PRINCIPAL MODEL.

### Required changes

RELAY must distinguish, where relevant:

- human principal
- workspace/organization
- team
- client application
- AI agent/session
- sub-agent
- adapter/tool identity
- external service identity

Important actions should preserve both the actor and the principal on whose behalf the actor operates. A shared generic "RELAY bot" identity is insufficient when better attribution is technically available.

## Attack 49 — Simple project roles are too coarse for team use

### Evidence

Role-based access control has decades of established use for simplifying organizational permission management, while NIST's ABAC model evaluates subject, object, operation, and environmental attributes. NIST zero-trust guidance also emphasizes dynamic, resource-specific policy rather than broad implicit trust.

### Verdict

USE ROLES FOR UX, ATTRIBUTES/RESOURCES FOR ENFORCEMENT.

### Required changes

RELAY may expose simple role templates, but authorization must be able to consider:

- workspace/team membership
- project/resource
- command/action
- data sensitivity
- client/agent identity
- delegated task scope
- command risk/effect metadata
- current project state/revision
- time/expiry or other relevant conditions

A role must not become a universal "can do anything in this workspace" shortcut.

## Attack 50 — Delegation and impersonation are different security events

### Evidence

RFC 8693 explicitly distinguishes delegation from impersonation. Delegation can preserve both the subject and the actor, which is important for accountability.

### Verdict

NEW DELEGATION CONTRACT.

### Required changes

Agent delegation should:

- preserve the delegation chain
- never silently widen scope
- retain project/resource boundaries
- carry expiry/budget limits where applicable
- remain revocable
- require explicit permission before further delegation
- remain attributable in audit history

If an external service forces impersonation semantics, RELAY should record that loss of attribution as a limitation rather than pretending delegation was preserved.

## Attack 51 — Shared credentials can silently bypass team offboarding

### Evidence

GitHub warns that a deploy key can continue to provide repository access to anyone holding the private key even after the user who created it is removed from the organization. More generally, membership removal and credential revocation are separate events.

### Verdict

MODIFY CONNECTION OWNERSHIP.

### Required changes

Every connection should have an explicit ownership scope, such as:

- personal
- workspace/team
- project/service

Offboarding must evaluate shared connections, active sessions, queued work, delegated agents, and temporary grants instead of assuming that removing a member ends all access.

## Attack 52 — Multi-project tokens need resource boundaries

### Evidence

RFC 8707 recommends audience-restricted access tokens and specifically discusses multi-tenant systems, noting that resource identifiers should distinguish tenants. RFC 9700 likewise recommends minimum privilege and audience restriction to reduce misuse of leaked tokens.

### Verdict

KEEP PROJECT ISOLATION, STRENGTHEN CREDENTIAL SCOPING.

### Required changes

Where external identity systems permit it, RELAY should prefer credentials/tokens that are:

- scoped to the intended resource/project
- minimally privileged
- short-lived where practical
- audience-restricted
- sender-bound where practical
- independently revocable

A credential valid for many unrelated projects is a wider-risk connection and should be visibly classified as such.

## Attack 53 — Approval can become stale between review and execution

### Evidence

MITRE's TOCTOU definition describes the general failure mode where resource state changes between a check and later use. Classic optimistic-concurrency work likewise treats validation of assumptions before commit as a core concurrency-control technique.

### Verdict

NEW REVISION/PRECONDITION REQUIREMENT.

### Required changes

Important writes and approved change plans should bind to relevant state, such as:

- project/base revision
- file/content hash
- expected entity/field value
- transaction revision
- expected integration state

If those assumptions no longer hold, RELAY returns a stale/conflict state and re-inspects instead of applying an old approval blindly.

## Attack 54 — Collaborative agents are often out of sync

### Evidence

SyncMind (ICML 2025) built 24,332 out-of-sync scenarios from 21 real repositories and found substantial recovery limitations across tested agents. Collaboration, when it happened, correlated positively with recovery, but the agents also showed very low collaboration willingness.

### Verdict

MAKE SYNCHRONIZATION EXPLICIT.

### Required changes

Multi-human/agent workflows need cheap synchronization primitives:

- project revision awareness
- task/resource ownership
- change-plan status
- conflict detection
- bounded claims/leases where useful
- merge/reconcile stage
- revalidation before write

The system should not assume that every agent has current project state merely because all agents started from the same repository.

## Attack 55 — More agents do not automatically mean better work

### Evidence

Recent multi-agent research reports both collaboration failure modes and cases where a single rogue/confused agent can degrade an entire system. Other 2026 work reports "collaboration degeneration" where one agent dominates and others become ineffective.

### Verdict

REJECT SWARM-BY-DEFAULT.

### Required changes

- multi-agent execution must be explicit and budgeted
- every worker needs bounded task/resource scope
- agent-to-agent chatter must be measured as a cost
- no child agent can inherit more authority than its parent
- critical/irreversible actions still pass the normal policy boundary
- default workflows should prefer the smallest number of agents that measurably improves the task

## Attack 56 — Local workspace membership is not enough for zero-trust team use

### Evidence

NIST SP 800-207 states that network location, asset ownership, or affiliation should not create implicit trust and recommends per-session, least-privilege resource access. SP 800-207A extends that model toward identity-centric application/service authorization.

### Verdict

MODIFY TEAM AUTHORIZATION.

### Required changes

Authorization should be evaluated for the requested project/resource and action rather than inferred from "user is logged in" or "agent is connected."

Policy should be refreshable when:

- membership changes
- external permissions change
- project policy changes
- client identity changes
- delegation expires
- risk/context changes materially

## Attack 57 — Audit logs become security data in team mode

### Evidence

NIST log-management guidance treats event generation, storage, access, retention, and disposal as security-management concerns.

### Verdict

MODIFY AUDIT DESIGN.

### Required changes

Important audit records should include enough attribution to reconstruct:

- delegating human/principal
- client
- agent/session
- project/resource
- command
- relevant revision/preconditions
- approval/change-plan
- result/verification
- time

Audit data itself requires access control, retention, and privacy rules. "Append-only" or "tamper evident" must not be claimed unless the chosen storage mechanism actually provides that property.

## Phase 0 collaboration/identity closure requirements

Phase 0 now also requires:

30. a principal model that separates human, client, agent, adapter, workspace, and project identities
31. role templates are backed by resource/attribute-aware policy rather than flat workspace-wide authority
32. delegation preserves actor/delegator scope and cannot silently widen
33. connection ownership and offboarding semantics are explicit
34. important writes/approvals have revision or equivalent preconditions
35. multi-agent workflows define synchronization/conflict primitives and cost budgets
36. audit records can reconstruct actor, delegator, project/resource, approval, and verification state


## Attack 58 — Durable workflow state does not make external effects exactly-once

### Evidence

AWS's current Durable Execution guidance explicitly distinguishes at-least-once and at-most-once retry semantics and states that neither automatically guarantees an external step runs exactly once across the whole workflow. Amazon's idempotent-API guidance describes the classic uncertain-outcome case where an operation may have succeeded even though the caller never received the response.

Recent systems work such as Fractal (NSDI 2026) likewise treats side-effectful commands as a separate recovery problem requiring explicit handling.

### Verdict

NEW DURABLE-EFFECT CONTRACT.

### Required changes

- durable job/checkpoint recovery and external-effect recovery are separate layers
- every side-effecting command declares retry/idempotency semantics
- retries reuse a stable logical request/idempotency key where supported
- a missing acknowledgement may become UNKNOWN_OUTCOME rather than FAILED
- recovery reconciles the external system before repeating an uncertain non-idempotent action

## Attack 59 — "Rollback" can be a dangerously dishonest word

### Problem

RELAY will coordinate editors, files, remote APIs, models, and third-party tools. Many of those effects cannot be atomically rolled back together.

### Verdict

MODIFY TRANSACTION TERMINOLOGY.

### Required changes

RELAY must distinguish:

- true rollback
- reliable inverse operation
- compensation
- restore from snapshot/backup
- manual recovery

A compensation may repair current state without erasing the original external event. UI and APIs must not promise rollback where only compensation is possible.

## Attack 60 — Retries can turn a small outage into duplicate work or a retry storm

### Evidence

AWS reliability guidance recommends verifying idempotency before retries, limiting retry calls, using timeouts/backoff, and avoiding unbounded retry behavior. Standard SQS queues explicitly provide at-least-once delivery and may deliver duplicates.

### Verdict

MODIFY JOB/QUEUE RECOVERY.

### Required changes

- retries are bounded and command-specific
- retryable writes require idempotency or explicit reconciliation
- duplicate delivery is expected by design
- queues expose backlog/degraded state
- repeated failure can enter blocked/dead-letter/manual-review state
- provider outage does not trigger infinite retries or repeated user approvals

## Attack 61 — A backup that has never been restored is not proven recovery

### Evidence

CISA's StopRansomware guidance recommends offline/encrypted backups and regular testing of backup availability and integrity in disaster-recovery scenarios. NIST SP 800-184 emphasizes recovery planning, playbooks, realistic testing, and improvement. CSF 2.0 specifically calls for verifying restoration assets before use and verifying restored assets before normal operation resumes.

### Verdict

NEW RESTORE-TEST REQUIREMENT.

### Required changes

- RELAY-owned durable state has documented backup/restore procedures
- restore/integrity tests are part of release and maintenance testing
- backup creation success is not displayed as "recovery verified"
- recovery playbooks define which data is rebuildable versus irreplaceable
- RELAY clearly states that it is not automatically the backup system for the user's game/project source

## Attack 62 — One RPO/RTO for all RELAY data is wasteful and misleading

### Evidence

NIST contingency-planning guidance defines Recovery Time Objective and Recovery Point Objective based on the impact/tolerance of the supported process.

### Verdict

MODIFY DATA-DURABILITY CLASSES.

### Required changes

Different RELAY state classes need different recovery targets.

Examples:

- rebuildable index/cache: tolerate loss, rebuild quickly
- transaction/approval state: low tolerated loss
- project policy/config: low tolerated loss
- raw evidence: retention-class dependent
- AI context cache: disposable

Do not pay premium durability cost for data that can be reconstructed.

## Attack 63 — Application downgrade does not imply data downgrade

### Evidence

Microsoft MSIX documentation warns that installing an older application version preserves app data and that data created by the newer app may not be backward compatible. PostgreSQL's upgrade documentation likewise describes cases where reverting requires a backup after the newer system has written to migrated/shared data.

### Verdict

NEW MIGRATION RECOVERY CONTRACT.

### Required changes

Before incompatible data/schema migration:

- preflight/check mode where practical
- record source schema/version
- create a verified recovery point when required
- durable migration progress/state
- define forward-recovery and rollback limits
- verify application health after migration
- block old binaries from opening data they cannot safely understand

Binary rollback and data rollback are separate operations.

## Attack 64 — Restarted is not recovered

### Evidence

NIST CSF 2.0 Recover calls for verifying restoration assets, verifying restored assets, and confirming normal operating status. NIST SP 800-160 Vol. 2 frames resilience as the ability to anticipate, withstand, recover, and adapt—not merely restart a process.

### Verdict

NEW RECOVERY TRUST STATES.

### Required changes

After crash/reboot/recovery RELAY may report:

- Reconciling
- Degraded
- Blocked
- Manual recovery required
- Healthy

"Healthy" requires relevant storage integrity, project-state reconciliation, adapter revalidation, and resolution/exposure of uncertain side effects.

## Attack 65 — Crash consistency needs deliberate fault injection

### Evidence

CrashMonkey research found previously unknown crash-consistency bugs even in mature file systems, and bounded black-box crash testing reproduced most known bugs in its target set while discovering additional data-loss/atomicity failures. OSDI 2025 continued active research on crash-consistent storage design.

### Verdict

MODIFY TEST STRATEGY.

### Required changes

Phase 1+ tests must kill RELAY at critical boundaries:

- before/after durable job checkpoint
- before/after external tool effect
- during result persistence
- during database/index update
- during migration
- during update/install
- under disk-full/write-failure conditions

Normal unit/integration tests are not sufficient evidence for recovery correctness.

## Attack 66 — Storage exhaustion can become a project-wide failure

### Problem

RELAY intentionally stores logs, screenshots, telemetry, transaction evidence, indexes, and AI results. A public user may run it for months.

### Verdict

NEW STORAGE-PRESSURE REQUIREMENT.

### Required changes

- enforce project/global quotas and retention policy
- monitor remaining storage
- protect critical transaction/recovery metadata from evidence growth
- enter an explicit degraded/read-mostly mode where practical before uncontrolled exhaustion
- never silently delete pinned or critical recovery state
- surface what can safely be pruned/rebuilt

## Attack 67 — Remote provider outage must not destroy local usefulness

### Evidence

NIST cyber-resilience guidance emphasizes continuing essential functions in adverse/degraded conditions. Distributed-systems guidance similarly recommends safe client behavior, bounded retries, and avoiding failure amplification.

### Verdict

KEEP LOCAL-FIRST CORE, ADD GRACEFUL DEGRADATION.

### Required changes

When remote AI/gateway services are unavailable:

- local deterministic inspection/audit remains available
- dashboard and stored results remain available
- remote-only jobs wait/fail clearly according to policy
- RELAY does not silently switch providers if that would change privacy/data policy
- retry/backlog state is visible and bounded

## Attack 68 — Recovery itself can restore bad or stale state

### Evidence

NIST CSF 2.0 specifically calls for checking backup/restoration integrity before use and verifying restored assets before normal operations resume.

### Verdict

NEW POST-RESTORE RECONCILIATION REQUIREMENT.

### Required changes

After database restore, migration rollback, or operational-state recovery:

- validate storage integrity
- re-check project and external-tool state
- invalidate stale capability/permission/provider caches
- rebuild derived indexes where appropriate
- resolve or surface in-flight/unknown operations
- only then declare normal operation restored

## Phase 0 resilience closure requirements

Phase 0 now also requires:

37. external effects have explicit retry/idempotency/unknown-outcome semantics
38. rollback, compensation, restore, and manual recovery are distinct concepts
39. durable queues/jobs have bounded retry and duplicate-delivery handling
40. RELAY backup design includes tested restore/integrity procedures
41. recovery targets differ by durability/data class
42. migration/update design separates binary rollback from data rollback
43. startup/recovery has explicit reconciling/degraded/healthy states
44. crash/fault injection is part of the implementation test plan
45. storage-pressure behavior and evidence quotas are defined
46. provider outages degrade capability without violating data/provider policy


## Attack 69 — Observability is not ground truth

### Evidence

Microsoft's Gray Failure work describes "differential observability": applications can be suffering while the system's failure detectors do not see a problem. NIST's 2026 post-deployment monitoring report likewise says robust monitoring practices remain immature and that real-world monitoring is needed precisely because controlled evaluations miss unexpected behavior.

### Verdict

NEW EVIDENCE-QUALITY CONTRACT.

### Required changes

RELAY must distinguish authoritative state, direct observation, derived measurement, corroborated finding, and inference.

A quiet monitor is not proof of health.

Findings should preserve the evidence class and source that justify them.

## Attack 70 — Absence of telemetry does not prove absence of an event

### Evidence

Distributed-tracing specifications and telemetry systems explicitly allow sampling, dropping, fragmentation, and conditional recording. Missing spans/events may therefore reflect collection behavior rather than system behavior.

### Verdict

MODIFY NEGATIVE-EVIDENCE SEMANTICS.

### Required changes

"Not observed" may be interpreted as "did not happen" only when the signal path is known complete for that event.

Evidence should expose when it is sampled, incomplete, dropped, expired, stale, or otherwise coverage-limited.

## Attack 71 — Sampling can erase the rare event RELAY most needs

### Evidence

OpenTelemetry treats sampling as a cost-control mechanism and warns that inconsistent decisions can produce unusable/fragmented traces. Tail sampling exists specifically because errors and high-latency traces can require selective retention.

### Verdict

MODIFY TELEMETRY COST CONTROL.

### Required changes

- sampling strategy/rate must be visible in evidence metadata
- exact counts cannot be inferred from sampled data unless a valid estimator applies
- important failures/rare events may need stronger retention policies
- audit/test mode may temporarily increase capture depth
- compact AI output must not hide that evidence was sampled

## Attack 72 — Telemetry can become its own performance bug

### Evidence

A 2025 Journal of Systems and Software study ran more than 5,000 experiments on instrumented containerized microservices and reported measurable performance degradation, with severe cases showing substantial throughput and latency effects. Other recent tracing studies report similarly non-trivial overhead.

### Verdict

NEW OBSERVABILITY-BUDGET REQUIREMENT.

### Required changes

- instrumentation modes need measured overhead budgets
- probes should activate only when needed
- performance diagnosis should compare with a minimally instrumented baseline where feasible
- RELAY must not report an instrumentation-induced slowdown as a project regression
- observability CPU/memory/network/storage cost belongs in usage metrics

## Attack 73 — Timestamps can fabricate causal stories

### Evidence

Recent 2026 distributed-AI tracing work shows that small clock skew can produce causally incorrect traces even when the system itself remains functionally correct. This is a recent preprint, so treat the precise thresholds as workload-specific, but the general distributed-systems problem is established.

### Verdict

MODIFY EVENT-ORDERING MODEL.

### Required changes

Prefer explicit causal links over wall-clock order:

- trace parent/child
- sequence numbers
- job/transaction IDs
- monotonic local ordering
- engine/session event order
- logical/causal links

Clock quality/skew becomes metadata where event ordering matters.

Do not claim "A caused B" merely because A's timestamp is earlier.

## Attack 74 — Multiple matching signals may still be one mistake

### Evidence

Recent anomaly-detection work shows correlated detector errors can create misleading apparent agreement. Multiple detectors firing on the same shared disturbance are not independent confirmation.

### Verdict

MODIFY EVIDENCE FUSION.

### Required changes

RELAY should track evidence lineage/dependency so that duplicated or correlated observations do not artificially inflate confidence.

Corroboration should reward independent evidence, authoritative confirmation, or controlled reproduction more than repeated views of the same underlying source.

## Attack 75 — False-positive rate alone does not measure operational usefulness

### Evidence

2026 reliability research argues that standard detector metrics can hide operationally intolerable false-alarm volume, and separate 2026 work shows repeated false alarms drive a cry-wolf effect and alarm fatigue.

### Verdict

MODIFY DETECTOR/ALERT EVALUATION.

### Required changes

For noisy automated checks measure where useful:

- absolute alerts per hour/session
- false discovery/noise rate
- repeated duplicate alerts
- user dismiss/override patterns
- confidence/calibration
- time-to-actionable finding

A detector with an impressive benchmark score but unusable alert volume is not a successful RELAY feature.

## Attack 76 — Metrics without uncertainty invite overconfidence

### Evidence

NIST AI 800-3 warns that common evaluation methods can rely on hidden assumptions or produce invalid uncertainty estimates and distinguishes fixed-benchmark performance from generalized performance.

### Verdict

NEW UNCERTAINTY REQUIREMENT.

### Required changes

RELAY should avoid presenting probabilistic/learned detector scores as objective truth.

Where material, expose:

- confidence/uncertainty
- sample size/coverage
- benchmark versus field evidence
- detector version
- calibration status
- known limitations

## Attack 77 — Monitoring systems themselves need monitoring

### Evidence

NIST SP 800-137 treats continuous monitoring as an ongoing assurance program whose effectiveness must itself be evaluated. Gray-failure work also shows the control plane can disagree with the actual application.

### Verdict

NEW META-OBSERVABILITY REQUIREMENT.

### Required changes

RELAY must track health of its collection path:

- dropped events/logs
- parser errors
- queue/backlog
- adapter/collector disconnect
- stale last-seen
- sampling state
- buffer overflow
- storage/quota pressure
- clock/time quality when relevant

A degraded telemetry path lowers the confidence of downstream findings.

## Attack 78 — Native diagnostics are authoritative but not eternal truth

### Problem

Engine-native validators/profilers are generally the best source for their own domain, but rule coverage, semantics, and behavior can change by version and may still contain bugs or gaps.

### Verdict

KEEP NATIVE-TOOL-FIRST WITH VERSIONED SEMANTICS.

### Required changes

Native findings retain:

- source/tool version
- check/rule identity/version where available
- scope/coverage
- session/revision
- timestamp

RELAY must not reinterpret an old native result as if it were produced by a newer engine/tool version.

## Attack 79 — Screenshots are partial observations, not state snapshots

### Problem

A screenshot can be valid but stale, cropped, from the wrong session, or visually omit hidden/editor state.

### Verdict

MODIFY VISUAL-EVIDENCE CONTRACT.

### Required changes

Capture metadata should include:

- target/window/viewpoint
- project/session/revision
- capture time
- resolution/crop
- capture method/version
- comparison baseline

Visual evidence cannot substitute for authoritative structured state when the question depends on hidden/nonvisual properties.

## Attack 80 — Correlation is not root cause

### Problem

RELAY will correlate changes, logs, metrics, entities, and failures. Correlation can rank hypotheses but does not prove causality.

### Verdict

MODIFY ROOT-CAUSE LANGUAGE.

### Required changes

Root-cause output should distinguish:

- observed symptom
- correlated change
- inferred hypothesis
- tested causal hypothesis
- verified cause

Where practical, high-confidence cause claims should be confirmed through reproduction, controlled change, dependency proof, or authoritative state.

## Attack 81 — Metrics can be gamed by RELAY itself

### Evidence

Goodhart-style failures are well documented: optimizing a measure can decouple the metric from the goal it originally represented.

### Verdict

MODIFY PRODUCT-METRIC GOVERNANCE.

### Required changes

Do not optimize one RELAY metric in isolation.

Examples:

- fewer AI calls can increase wrong decisions
- fewer tokens can remove essential evidence
- fewer alerts can hide failures
- higher cache hit rate can serve stale data

Product quality needs metric portfolios plus correctness/safety guardrails.

## Attack 82 — Telemetry identifiers and metadata are untrusted inputs

### Evidence

The W3C Trace Context security considerations warn that tracing metadata can be manipulated to cause monitoring denial, trace-ID collisions, and increased tracing cost if systems trust incoming context blindly.

### Verdict

EXTEND UNTRUSTED-INPUT BOUNDARY TO OBSERVABILITY.

### Required changes

- external trace IDs/correlation metadata are not trusted as authority
- normalize/validate telemetry identifiers
- apply resource limits to tracing/diagnostic requests
- preserve source/trust metadata
- do not allow external telemetry metadata to force unlimited capture
- suspicious observability data remains inspectable without changing RELAY policy

## Phase 0 observability/truth closure requirements

Phase 0 now also requires:

47. evidence/results carry source, freshness, revision, and quality/completeness metadata where relevant
48. missing/sampled telemetry is not treated as negative proof
49. observability overhead and capture budgets are benchmarked
50. event ordering/causality does not rely on wall-clock timestamps alone
51. detector evaluation includes operational alert burden and calibration
52. observability-pipeline health affects downstream confidence
53. root-cause language distinguishes correlation/inference from verified cause
54. visual evidence is revision/session aware
55. product metrics use multi-metric guardrails rather than one optimization target
56. telemetry/correlation metadata is treated as untrusted input


## Attack 83 — RELAY can become the complexity it was created to remove

### Evidence

A 2026 multivocal review of platform engineering found that internal developer platforms are intended to reduce cognitive load but can themselves add a learning curve and become a source of complexity when over-engineered. The review also notes that evidence for many platform-specific practices remains immature. A broader systematic mapping study of developer cognitive load found a substantial research base linking programming tasks and cognitive load, while emphasizing measurement challenges.

### Verdict

NEW SIMPLICITY CONSTRAINT.

### Required changes

- cognitive/user complexity becomes a first-class architecture constraint
- common workflows must be measured by time, steps, decisions, concepts, and failure recovery
- new platform features must justify the user/maintenance complexity they add
- every milestone includes an explicit simplification pass
- public single-user use remains the default mental model unless team features are actually needed

## Attack 84 — Security/privacy configuration should not become the user's full-time job

### Evidence

CISA/NSA/FBI Secure-by-Design guidance states that security complexity should not be pushed onto customers and that the secure path should be the default path. CISA repeatedly recommends secure configurations "out of the box" rather than requiring customers to spend additional effort hardening products.

### Verdict

MODIFY SECURITY/PRIVACY UX.

### Required changes

- safe privacy, credential, adapter, retry, and evidence-retention defaults ship enabled
- first-run setup does not ask users to design a security architecture
- Advanced settings explain deviations from recommended defaults
- dangerous combinations are blocked or require explicit informed override
- basic security/observability necessary for RELAY operation is not hidden behind paid or expert-only configuration

## Attack 85 — "Fewer choices is always better" is also too simplistic

### Evidence

A 2010 meta-analysis of choice overload found a near-zero mean effect across 50 experiments with substantial variation, while a 2015 meta-analysis found that complexity of the choice set, task difficulty, preference uncertainty, and decision goal moderate overload.

### Verdict

MODIFY CHOICE DESIGN.

### Required changes

RELAY should not enforce an arbitrary global cap on options.

Instead:

- provide strong defaults
- expose choices contextually
- group/search/filter larger sets
- explain recommended choices
- reveal advanced alternatives when relevant
- test choice burden on real RELAY tasks

The target is lower decision burden, not minimal option count.

## Attack 86 — Configuration flexibility can create an untestable product

### Evidence

Research on highly configurable systems repeatedly finds that configuration spaces grow combinatorially and are difficult to test comprehensively. An empirical study of JHipster configurations found 35.7% of evaluated configurations failed and showed that even systematic sampling can exceed practical test budgets. A 2023 multiple-case study likewise found testing highly configurable systems challenging and configuration dependencies often only partially modeled.

### Verdict

NEW CONFIGURATION-BUDGET REQUIREMENT.

### Required changes

- every user-visible configuration option is treated as additional product state
- configuration interactions belong in the test plan
- supported/recommended profiles are allowed instead of pretending all combinations are equally validated
- configuration schemas should encode constraints/defaults/dependencies
- unsupported combinations fail early and clearly
- internal implementation knobs are not automatically public settings

## Attack 87 — Invalid configuration often fails too late

### Evidence

OSDI 2016 Best Paper research on latent configuration errors found mature systems frequently failed to validate critical configurations during initialization; generated early checks detected more than 75% of studied real-world latent configuration errors.

### Verdict

MODIFY STARTUP/ONBOARDING VALIDATION.

### Required changes

- validate configuration and integration assumptions as early as practical
- a project should not appear fully Ready when important configuration has not been exercised/validated
- run startup/preflight checks for values that otherwise fail only during rare recovery or integration paths
- diagnostics should identify the setting and consequence in plain language

## Attack 88 — AI assistance can create verification fatigue

### Evidence

A CHI 2026 study of 60 developers held the model backend fixed while varying AI interaction style and found that interface design materially changed time, correctness, workload, and verification burden. Verification load also tracked stress/fatigue over repeated use.

### Verdict

MODIFY AI/DASHBOARD INTERACTION DESIGN.

### Required changes

- RELAY evaluates not only model quality but verification work imposed on the human
- common results should contain enough evidence to verify without opening raw logs
- interaction style may vary by task complexity rather than using one universal chat pattern
- repeated approvals/reviews/AI output checking become measurable UX cost

## Attack 89 — Safety controls can trigger security fatigue

### Evidence

NIST's 2016 Security Fatigue study found more than half of interviewed participants expressed security fatigue, with resignation, decision avoidance, and tendency toward easier choices among the outcomes. NIST's 2026 human-centered cybersecurity work continues to emphasize human factors as part of cybersecurity outcomes.

### Verdict

MODIFY CONTROL SURFACE.

### Required changes

- reduce unnecessary decisions/prompts
- batch coherent approvals
- provide secure defaults
- hide controls that are not relevant to the active task
- measure repeated prompt/override behavior
- treat high dismissal rates as a product-design problem, not user failure

## Attack 90 — Feature accumulation creates permanent technical debt

### Evidence

The 2015 Hidden Technical Debt in Machine Learning Systems paper identifies configuration issues, boundary erosion, entanglement, undeclared consumers, and system-level anti-patterns as major sources of long-term maintenance cost. Older empirical maintenance research likewise links software complexity and downstream maintenance performance.

### Verdict

NEW FEATURE-ADMISSION AND RETIREMENT POLICY.

### Required changes

A core feature needs evidence that value exceeds:

- implementation cost
- test/configuration-space expansion
- security/privacy surface
- migration/version burden
- docs/UI/CLI burden
- background/runtime overhead
- support cost

RELAY must also be willing to remove, merge, or demote features.

## Attack 91 — Premature universal abstraction can hide rather than remove complexity

### Evidence

Brooks' classic "No Silver Bullet" argues that software's essential conceptual complexity cannot be removed merely by changing representations. RELAY risks creating a generic "all engines" abstraction before enough real integrations exist to reveal what is actually common.

### Verdict

MODIFY GENERALIZATION STRATEGY.

### Required changes

- build clean boundaries without pretending the first adapter defines a universal engine model
- keep engine-specific semantics behind adapters
- validate a core abstraction against at least one materially different second integration before declaring it stable
- escape-hatch proliferation is evidence the abstraction is premature
- generic APIs must earn their existence through repeated concrete use

## Attack 92 — RELAY can generate operational toil of its own

### Evidence

Google SRE defines toil as repetitive, predictable operational work and explicitly recommends measuring/limiting it because maintenance activity can consume a team if left unchecked.

### Verdict

NEW OPERABILITY/TOIL BUDGET.

### Required changes

Measure repetitive user/maintainer work such as:

- reconnecting integrations
- repairing updates
- configuration edits
- permissions cleanup
- reindexing
- compatibility triage
- repeated approvals
- support diagnosis

Recurring safe toil should be automated or eliminated at the root rather than normalized as "how RELAY works."

## Attack 93 — Documentation and advanced settings can become another fragmented toolchain

### Evidence

A 2025 empirical study of Apache Airflow workflows found defining/executing workflows were major challenges, often involving configuration errors, and developers relied on diverse documentation and expertise. NIST's 2024 survey on human-centered cybersecurity also found practitioners face challenges translating research into practice.

### Verdict

MODIFY DOCUMENTATION/DIAGNOSTICS.

### Required changes

- shared semantic sources generate/validate CLI help, schemas, dashboard metadata, and AI skill references where practical
- task-oriented human documentation remains concise and editorial
- common failures route through one self-diagnostic path such as a future relay doctor
- error output gives a clear next action instead of requiring users to search several documents
- current version/capability state is visible in diagnostics

## Attack 94 — Progressive disclosure can hide complexity without actually reducing it

### Problem

A UI can put 200 controls behind an Advanced button and still leave the underlying product conceptually incoherent.

### Verdict

MODIFY PROGRESSIVE-DISCLOSURE RULE.

### Required changes

Simple, detailed, and advanced views must share the same stable concepts/names.

Advanced views reveal evidence and control; they should not expose a second unrelated mental model.

If users routinely require Advanced to accomplish normal work, the default product surface is wrong.

## Attack 95 — Enterprise architecture can poison the personal-user experience

### Problem

RELAY now has concepts for organizations, delegation chains, provider profiles, data classes, recovery policy, adapter provenance, audit policy, and team authorization. Most solo creators should not need to learn those concepts.

### Verdict

SEPARATE CAPABILITY FROM PRESENTATION.

### Required changes

- personal installations can use implicit sensible workspace/policy defaults
- team/enterprise concepts stay hidden until activated
- optional subsystems are lazy
- data structures may remain future-capable without forcing enterprise terminology into onboarding
- public v0.x success is measured first on personal creator workflows

## Attack 96 — More automation can still mean more mystery

### Evidence

CISA Secure-by-Design guidance recommends field testing how customers actually deploy products and making the secure route the easiest route. Configuration-error research likewise shows unchecked assumptions can remain latent until failure.

### Verdict

MODIFY AUTO-DETECTION.

### Required changes

Automatic detection/setup must be:

- visible
- explainable
- correctable
- validated
- cheap to reset/re-run

"RELAY detected this" should always have a path to "show me why" and "change it."

## Attack 97 — The default path must deliver value before the user configures the ecosystem

### Problem

If the first useful audit requires an AI provider, cloud account, custom adapter, data policy wizard, team setup, and ten permission screens, RELAY has failed its original mission even if the architecture is secure.

### Verdict

NEW TIME-TO-FIRST-VALUE REQUIREMENT.

### Required changes

Phase 1+ should benchmark a minimal personal golden path:

1. install
2. detect/add supported project
3. run useful deterministic local inspection/audit
4. show result

Remote AI and advanced integrations should improve the experience, not be prerequisites for basic value.

## Attack 98 — Complexity needs an explicit budget, not good intentions

### Verdict

NEW COMPLEXITY-GOVERNANCE REQUIREMENT.

### Required changes

Every milestone tracks a portfolio including:

- steps/time to first useful result
- mandatory decisions/concepts
- configuration count
- supported profiles/combinations
- background components
- compatibility matrix size
- install/update failure rate
- support/diagnostic time
- Advanced-setting usage
- RELAY-management time versus project work

A milestone can fail on complexity regression even when all functional tests pass.

## Phase 0 simplicity/operability closure requirements

Phase 0 now also requires:

57. simplicity/cognitive load is a first-class product constraint
58. secure/privacy/recovery defaults minimize required user hardening
59. configuration growth is governed as testable product state
60. startup/preflight validation catches important configuration failures early
61. AI UX is evaluated for human verification burden
62. feature admission includes long-term support/test/configuration cost
63. engine abstraction remains evidence-driven rather than prematurely universal
64. repetitive RELAY toil is measured and targeted for elimination
65. common diagnostics have a single low-friction entry point
66. personal-user UI hides irrelevant enterprise/team complexity
67. the minimal golden path produces useful local value without remote AI
68. complexity-budget metrics are reviewed at every milestone


## Attack 99 — Official UEFN automation support does not erase broader Epic terms

### Evidence

Epic's current September 2026 Terms of Service list UEFN as a Licensed Product and prohibit bot software/services used to automate Licensed Products. At the same time, Epic's August 2026 UEFN documentation expressly ships Unreal MCP so agentic coding tools can connect to and drive the editor.

### Verdict

NARROW AUTOMATION TO DOCUMENTED DEVELOPER SURFACES.

### Required changes

- treat Unreal/UEFN MCP and other documented developer automation surfaces as the intended supported path
- do not interpret editor automation support as permission to automate Fortnite gameplay/player-client use generally
- RELAY must not automate gameplay, evade integrity systems, or control the Fortnite client outside documented development/testing workflows
- if RELAY later depends on broader automation, obtain legal/official clarification before public release
- record the Epic terms/documentation versions used for the compatibility decision

## Attack 100 — A runtime bridge cannot "phone home" from published UEFN content

### Evidence

Current UEFN Supplemental Terms state that Developer-Made Content/code may not attempt to establish connections to servers outside Epic hosting after upload/download by end users.

### Verdict

KEEP RUNTIME OBSERVABILITY, REJECT EXTERNAL IN-ISLAND RELAY NETWORKING.

### Required changes

- runtime instrumentation uses supported Verse/UEFN logs, debug/session tools, or Epic-hosted mechanisms
- local development bridges are excluded from published content
- publishing validation checks RELAY instrumentation for prohibited external networking
- RELAY documentation must not suggest that creators embed a persistent external telemetry client inside published islands

## Attack 101 — Public RELAY branding cannot casually rely on Epic's Fan Content Policy

### Evidence

Epic's current Fan Content Policy defines covered Epic-related apps/sites as personal, non-commercial, and freely accessible and restricts Epic marks from identifying/promoting another product or business.

### Verdict

SEPARATE RELAY BRANDING FROM EPIC IP.

### Required changes

- RELAY's product name/logo/identity stay independent of Epic/Fortnite/Unreal marks
- use UEFN/Fortnite/Unreal names descriptively for compatibility, not as product branding
- do not use Epic logos or imply endorsement/certification without permission
- commercial RELAY must not assume the Fan Content Policy authorizes its branding
- public marketing gets a terms/trademark review before release

## Attack 102 — Bundling third-party applications creates unnecessary license/trademark risk

### Evidence

Blender and Krita permit broad use/redistribution under GPL terms, while Epic's tools/assets operate under proprietary licenses and product-specific terms. Bundling also creates update, security, notice, and trademark obligations.

### Verdict

DETECT, DO NOT BUNDLE BY DEFAULT.

### Required changes

- public RELAY installer detects user-installed UEFN/Blender/Krita
- do not bundle UEFN/Fortnite/Epic proprietary binaries/assets
- do not bundle modified Blender/Krita builds unless RELAY explicitly accepts the redistribution/source/trademark obligations
- third-party installers are obtained from official sources where practical
- RELAY-owned packages include only dependencies with known redistribution rights

## Attack 103 — Blender/Krita companion plugins can impose copyleft obligations

### Evidence

Blender's official license page says published Python add-ons using Blender's Python API must use a GPL-compatible license. Krita's official license page states distributed plugins using its extension API must be GPL. Unreal Engine's current EULA separately identifies GPL and certain share-alike licenses as non-compatible when they would impose those terms on Epic Licensed Technology.

### Verdict

NEW LICENSE-BOUNDARY REQUIREMENT.

### Required changes

- distributed Blender/Krita companions are licensed according to their host's GPL requirements
- do not share/copy GPL implementation code into differently licensed Unreal/UEFN companions or RELAY Core without compatibility review
- maintain process/protocol boundaries where they help separate differently licensed components
- do not claim that separate processes automatically settle derivative-work questions
- license/distribution architecture receives legal review before public release

## Attack 104 — UEFN/Unreal companion code has its own distribution restrictions

### Evidence

The current Unreal Engine EULA restricts combining Licensed Technology with non-compatible licenses and imposes specific distribution paths for Engine Tools.

### Verdict

MODIFY UEFN COMPANION DESIGN.

### Required changes

- classify whether any RELAY UEFN/Unreal companion includes, links, or constitutes Engine Tools/Licensed Technology
- choose companion license/distribution channel only after that classification
- prefer external documented protocol integration when it avoids unnecessary license coupling
- do not assume RELAY Core's eventual license automatically applies to every in-editor component

## Attack 105 — "Developer-made content is yours" does not mean every project asset is yours

### Evidence

Epic's UEFN terms say Developer-Made Content remains the developer's apart from Epic/third-party rights, but the developer warrants they have rights sufficient for Epic's license. Fortnite Developer Rules likewise require creators to own or obtain the necessary rights and separately restrict Epic-owned IP not made available for creator use.

### Verdict

NEW CONTENT-PROVENANCE REQUIREMENT.

### Required changes

The asset/content registry should track where practical:

- source/creator
- license/rights basis
- Epic-owned asset status
- third-party marketplace/content status
- attribution obligations
- export/redistribution restrictions
- AI-generated/assisted provenance

Missing or conflicting provenance can block a configured publish workflow, but RELAY cannot itself adjudicate ownership.

## Attack 106 — AI output rights are not the same as copyright ownership

### Evidence

The U.S. Copyright Office's January 2025 Part 2 report concludes that generative-AI outputs are copyrightable only where sufficient human-authored expressive elements exist; prompts alone are generally insufficient, while human-authored material, selection/arrangement, or creative modification may qualify.

### Verdict

REJECT AUTOMATIC OWNERSHIP CLAIMS.

### Required changes

RELAY must not tell users:

- AI-generated output is automatically copyrighted
- prompts alone guarantee copyright
- AI-generated output is automatically public domain
- AI-generated output is guaranteed non-infringing

Track generation/human-edit provenance where useful and separate provider contractual usage rights from copyrightability.

## Attack 107 — Open-source license compliance is a release artifact, not an afterthought

### Evidence

NIST's software-supply-chain guidance emphasizes machine-readable component inventories, provenance, and open-source controls.

### Verdict

NEW LICENSE-INVENTORY GATE.

### Required changes

Every distributed RELAY artifact should have a reproducible inventory of:

- component/version
- license
- copyright notice
- attribution/license-text obligations
- source/source-offer obligations
- copyleft/linking concerns
- redistribution restrictions
- companion-component licenses

Unknown or incompatible licenses block release until resolved.

## Attack 108 — Security SBOM and legal license inventory are related but not identical

### Problem

An SBOM can identify packages without proving license compatibility, notice compliance, trademark permission, or asset redistribution rights.

### Verdict

SEPARATE LEGAL METADATA FROM SECURITY METADATA.

### Required changes

- reuse shared component/provenance data where possible
- track license/notice/source obligations separately from vulnerabilities
- include non-code assets, fonts, music, templates, and companion plugins where relevant
- do not interpret "no vulnerabilities" as "clear to redistribute"

## Attack 109 — Privacy/legal promises can become enforceable product risk

### Evidence

FTC guidance repeatedly warns software developers to honor privacy/security claims and make user choices clear. FTC guidance on privacy-enhancing technologies also warns that products must not overstate the privacy guarantees of a particular implementation.

### Verdict

ALIGN MARKETING WITH TESTED BEHAVIOR.

### Required changes

- "local-only", "private", "encrypted", "does not upload", and similar claims need test evidence and precise limitations
- privacy notice, telemetry behavior, diagnostics, provider profiles, and dashboard wording must agree
- product marketing cannot promise stronger privacy/security than RELAY actually enforces
- legal/privacy copy is version-controlled alongside behavior-changing releases

## Attack 110 — Platform and license terms can change underneath RELAY

### Evidence

Epic's UEFN terms expressly incorporate other policies and documentation that Epic may update; Epic maintains an active Fortnite Developer Rules change log. RELAY also depends on evolving third-party licenses/terms and AI provider policies.

### Verdict

NEW TERMS-COMPATIBILITY LIFECYCLE.

### Required changes

Track for material integrations:

- source document/URL
- effective/update date when available
- last RELAY review date/version
- assumptions derived from it
- current compatibility status

A material terms/license change creates a review task and may temporarily mark publishing/distribution compatibility Unknown/Needs Review rather than silently assuming old conclusions remain valid.

## Attack 111 — Automated compliance checks can create false legal confidence

### Problem

RELAY can mechanically detect missing notices, unknown licenses, policy-version drift, or configured rule violations. It cannot determine all copyright ownership, fair use, trademark permission, contract interpretation, or jurisdiction-specific legal obligations.

### Verdict

MODIFY COMPLIANCE LANGUAGE.

### Required changes

Use statuses such as:

- metadata missing
- known conflict
- policy changed
- review required
- blocked by configured policy

Do not output "legally compliant", "copyright cleared", "Epic approved", or "guaranteed non-infringing" unless an authoritative process truly establishes that fact.

## Attack 112 — RELAY's own open-source license can constrain future integrations/business models

### Problem

A RELAY Core license selected too early could conflict with Unreal companion distribution, paid/hosted services, contribution policy, or the desired separation from GPL host plugins.

### Verdict

DEFER LICENSE SELECTION UNTIL BOUNDARIES ARE PROTOTYPED.

### Required changes

Phase 1 legal/license research should evaluate:

- permissive versus reciprocal options
- cloud/hosted implications
- contributor copyright model
- SDK/adapter license
- Unreal/UEFN companion compatibility
- Blender/Krita GPL companion separation
- dependency license set
- commercial/public roadmap

Do not pick a license merely to fill the LICENSE file.

## Phase 0 legal/licensing closure requirements

Phase 0 now also requires:

69. UEFN automation scope is explicitly limited to documented developer surfaces unless clarified otherwise
70. published UEFN instrumentation avoids prohibited external networking
71. RELAY branding/marketing does not depend on Epic Fan Content permission for commercial use
72. third-party application bundling is opt-in/legally reviewed rather than default
73. Blender/Krita GPL companion obligations and Unreal non-compatible-license boundaries are documented
74. asset/content provenance and license metadata are part of the asset model
75. AI-generated content UI avoids unsupported copyright/ownership claims
76. dependency/license/notice inventory is a public-release gate
77. privacy/security marketing claims are tied to tested behavior
78. material platform/license terms are tracked as versioned external dependencies
79. automated compliance checks avoid legal-certification language
80. RELAY Core/SDK/companion license choice remains open until integration boundaries are prototyped


## Attack 113 — Semantic Versioning is a promise, not a compatibility oracle

### Evidence

Large empirical studies across Maven, Java, and Go ecosystems consistently find that breaking changes occur outside major releases. A 2024 extended Maven study found 11.58% of tested dependency updates caused client-affecting breaking changes and almost half of those occurred in non-major updates. A large Go study found 28.6% of no-major upgrades introduced breaking changes.

### Verdict

KEEP VERSION NUMBERS, REJECT VERSION-NUMBER-ONLY TRUST.

### Required changes

- SemVer may communicate RELAY release policy
- automated compatibility tests determine actual compatibility
- capability/schema negotiation beats assumptions derived from a version string
- release tooling detects breaking contract changes before publication
- third-party adapter versions are not considered safe solely because their SemVer suggests compatibility

## Attack 114 — Breaking changes are behavioral, not just syntactic

### Evidence

Microsoft's API guidelines explicitly count changes in behavior, error contracts, permissions, pagination, rate limits, latency, and concurrency among changes that may break clients. API-evolution research also shows client breakage arises from maintenance/feature behavior, not only renames.

### Verdict

EXPAND RELAY'S DEFINITION OF BREAKING CHANGE.

### Required changes

Compatibility review includes:

- command behavior/side effects
- error/exit semantics
- permissions
- idempotency/retry semantics
- privacy/egress defaults
- ordering/pagination
- performance/rate/concurrency guarantees relied on by clients
- structured schemas and names

A source-compatible change can still be operationally breaking.

## Attack 115 — Clients will not all update together

### Evidence

Protocol Buffer best practices explicitly warn that clients and servers are never updated at exactly the same time and may be rolled back. Kubernetes maintains a formal version-skew policy because real systems operate with mixed component versions.

### Verdict

REJECT LOCKSTEP UPGRADE ASSUMPTIONS.

### Required changes

Every durable RELAY interface defines:

- supported version skew
- negotiation/handshake behavior
- safe read-only fallback
- fail-closed write behavior when incompatible
- rolling upgrade order
- rollback limitations

Dashboard, CLI, local host, gateway, adapters, and companion plugins must be tested in mixed-version states.

## Attack 116 — Serialization choice changes what "compatible" means

### Evidence

Protocol Buffers documentation explicitly differentiates binary wire safety from ProtoJSON safety. Unknown fields, field names, enum representation, and field-number reuse can produce very different compatibility behavior across encodings.

### Verdict

NEW SCHEMA-EVOLUTION RULESET.

### Required changes

- choose serialization/IDL only after defining the compatibility requirements
- test each supported encoding separately
- never reuse schema identifiers where the format warns against it
- reserve deleted identifiers when supported
- define unknown-field behavior
- additive fields do not automatically mean all old JSON clients are safe
- persisted data and IPC schemas have separate compatibility tests if representations differ

## Attack 117 — Saved results can outlive their interpretation code

### Problem

RELAY intentionally stores durable result IDs, transactions, evidence, and history. A result created years earlier may be opened by code whose rules, adapter semantics, or data schema have changed.

### Verdict

NEW HISTORICAL-SEMANTICS REQUIREMENT.

### Required changes

Durable results retain:

- producing RELAY version
- result schema version
- command/rule/adapter version
- project/session revision
- relevant raw evidence reference/version

New code must not silently reinterpret old fields according to new semantics. When faithful rendering is impossible, RELAY says so.

## Attack 118 — Deprecation tags alone do not move users

### Evidence

Empirical API-deprecation research found clients often lag in reacting to deprecated APIs, while older semantic-versioning studies found deprecation tags were inconsistently applied. Microsoft and Kubernetes both impose explicit deprecation/support policies instead of assuming users migrate immediately.

### Verdict

MODIFY DEPRECATION LIFECYCLE.

### Required changes

Deprecation includes:

- explicit status
- replacement/migration path
- earliest removal date/support rule
- owner
- compatibility impact
- usage signal where privacy-safe
- generated warnings/docs/skills

Elapsed time alone does not prove safe removal when known clients still depend on the contract.

## Attack 119 — API migration needs tooling, not only prose

### Evidence

A 2023 MOBILESoft study generated migration guides across 13 web-service version increments and identified 1,132 breaking changes; some changes remained unsolvable automatically and required developer-authored migration guidance.

### Verdict

NEW MIGRATION-ASSISTANCE REQUIREMENT.

### Required changes

For important public breaking changes RELAY should provide, where practical:

- compatibility scanner
- config/data migration
- command/flag rewrite hints
- regenerated AI skills/wrappers
- adapter manifest migration guidance
- machine-readable deprecation/replacement metadata
- explicit manual steps for changes that cannot be automated

"See changelog" is insufficient for major workflow migrations.

## Attack 120 — Compatibility shims can fossilize the architecture

### Problem

Keeping every old command, schema, flag, and behavior forever makes the core larger, harder to secure, harder to test, and more expensive for AI clients to understand.

### Verdict

NEW COMPATIBILITY-DEBT BUDGET.

### Required changes

Every shim/alias needs:

- owner
- supported versions
- tests
- usage/compatibility signal
- removal condition
- review date

Old compatibility commands do not remain in default AI capability context merely because they still exist.

## Attack 121 — Feature flags accumulate into long-term state

### Evidence

A 2026 longitudinal study of more than 4,000 feature-toggle events in Kubernetes and GitLab found removals lagged additions and some flags became effectively permanent, with Kubernetes flags having a median lifespan of 734 days in the studied data.

### Verdict

MODIFY FEATURE-FLAG GOVERNANCE.

### Required changes

Flags used for compatibility/rollout need:

- owner
- introduction release
- purpose/default
- rollout state
- compatibility implications
- removal condition
- review/expiry target

Feature flags cannot become an undocumented permanent versioning system.

## Attack 122 — Human-readable CLI output is a dangerous machine API

### Problem

Agents and shell scripts will be tempted to parse pretty CLI text because it is easy. Tiny wording improvements then become accidental breaking changes.

### Verdict

SEPARATE HUMAN AND MACHINE CONTRACTS.

### Required changes

- human output is not a stable parse contract
- structured output has explicit schema/contract version
- scripts/skills use structured mode
- exit codes have stable documented categories
- presentation can evolve without silently breaking automation
- deprecated fields/commands provide structured migration metadata

## Attack 123 — Version sniffing is weaker than capability negotiation

### Problem

Two peers can have the same nominal version but different build flags, adapters, host capabilities, experimental features, or external-tool versions.

### Verdict

USE VERSION + CAPABILITY NEGOTIATION.

### Required changes

Handshake/state includes both:

- version/protocol range
- actual capabilities

Clients ask what is available rather than maintaining giant hard-coded version tables where negotiation is possible.

## Attack 124 — Stable schema identifiers are permanent historical baggage

### Evidence

Protocol Buffers warns strongly against reusing field/tag numbers because serialized historical data or older code may still exist. Kubernetes similarly prioritizes API round-tripping and versioned removal.

### Verdict

RESERVE RETIRED IDENTIFIERS.

### Required changes

For schema technologies with stable numeric/name identifiers:

- do not recycle retired IDs
- reserve removed IDs/names when supported
- keep migration/schema history sufficient to interpret older state
- design ID spaces expecting long product lifetimes

Saving a few identifier numbers is not worth silent corruption.

## Attack 125 — Security fixes may need to break compatibility

### Problem

An unsafe behavior can become a public contract. Preserving it forever can conflict with RELAY's security/privacy guarantees.

### Verdict

ALLOW EXPLICIT EMERGENCY BREAKS.

### Required changes

Define a security-breaking-change process covering:

- severity/decision authority
- affected versions
- mitigation
- replacement/migration
- communication
- rollback limits
- support window exceptions

Do not hide a behavior break inside a patch release without clearly communicating the client impact.

## Attack 126 — Infinite support promises are another form of technical debt

### Evidence

Mature systems such as Kubernetes maintain explicit support/version-skew/deprecation windows rather than supporting every historical component forever. Microsoft API guidance likewise requires version/deprecation planning and support status for previous versions.

### Verdict

NEW SUPPORT-LIFECYCLE REQUIREMENT.

### Required changes

Before RELAY reaches stable public releases, define:

- supported release lines
- security-fix policy
- protocol/SDK support window
- data-migration source window
- CLI/API deprecation policy
- adapter/skill compatibility support

Support policy must reflect resources RELAY can actually sustain.

## Attack 127 — Experimental surfaces need weaker promises than stable ones

### Problem

If every preview adapter/command immediately receives full backward-compatibility guarantees, RELAY will freeze immature designs and repeat the premature-abstraction problem.

### Verdict

NEW STABILITY-CLASS MODEL.

### Required changes

Classify interfaces such as:

- internal
- experimental
- preview
- stable

Each class has explicit change/deprecation promises.

Experimental does not mean unversioned or unsafe; it means the compatibility promise is intentionally narrower.

## Attack 128 — Compatibility itself can bloat AI context and diagnostics

### Problem

Legacy commands, schemas, aliases, and adapter versions can reintroduce the context/tool bloat RELAY was designed to avoid.

### Verdict

KEEP LEGACY SUPPORT OFF THE DEFAULT PATH.

### Required changes

- capability discovery returns current/relevant commands by default
- legacy aliases are invoked only for clients that need them
- deprecated skill/docs are not always loaded
- dashboard hides retired/legacy contracts unless troubleshooting/migrating
- compatibility overhead is measured in token/runtime/maintenance cost

## Phase 0 versioning/compatibility closure requirements

Phase 0 now also requires:

81. stable contracts have explicit owners and stability classes
82. actual compatibility is tested rather than inferred from SemVer
83. breaking-change definitions include behavior, errors, permissions, privacy, and relevant performance contracts
84. mixed-version/version-skew behavior is defined
85. structured schemas have documented forward/backward evolution rules
86. durable historical results retain enough version/provenance metadata to interpret safely
87. deprecation includes migration metadata and a bounded support/removal process
88. public breaking changes have practical migration assistance where feasible
89. compatibility shims and feature flags have owners/removal criteria
90. human CLI presentation is not the machine-readable automation contract
91. version and capability negotiation are both supported where appropriate
92. retired schema identifiers are not reused when the chosen format makes reuse unsafe
93. emergency security-breaking-change policy exists
94. stable public release support windows are bounded and documented
95. experimental/preview/stable surfaces have different compatibility promises
96. legacy compatibility surfaces stay out of normal AI/user context unless needed


## Attack 129 — Cheap AI can still produce an expensive workstation

### Evidence

UEFN's current recommended PC profile calls for 32 GB RAM or more, 8 GB or more VRAM, a DX12 GPU, and NVMe storage. RELAY will coexist with an already resource-intensive editor and potentially a Fortnite play session.

OSDI 2026 consumer-GPU work (Nixie) specifically targets the problem of large ML working sets coexisting on consumer GPUs and reports severe memory-sharing inefficiencies under competing applications. Mobile OSDI 2026 work (SERENO) independently shows background LLM inference can materially degrade a foreground interactive workload under memory-bandwidth contention; the exact mobile measurements do not directly transfer to desktop UEFN, but the interference mechanism is relevant.

### Verdict

NEW FOREGROUND-RESOURCE CONTRACT.

### Required changes

- foreground creator workload wins over optional RELAY/background AI work
- local inference is not started merely because it saves API tokens
- active UEFN/Fortnite/Blender/Krita resource pressure influences scheduling
- performance impact is measured in user-facing latency/frame time as well as model throughput
- local/cloud/hybrid routing includes foreground-interference cost

## Attack 130 — Hidden background work is still foreground interference

### Evidence

Windows Search deliberately backs off indexing when the user is active or the machine is busy. Windows EcoQoS is explicitly intended for work that is not part of the foreground user experience and trades peak performance for power/thermal efficiency. Classic operating-systems research on interactive performance and background work reaches the same broad conclusion: best-effort work should yield to latency-sensitive activity.

### Verdict

NEW BACKGROUND-BACKOFF REQUIREMENT.

### Required changes

Background RELAY work should:

- use OS-supported low-priority/QoS mechanisms where useful
- slow/pause under foreground contention
- resume when headroom returns
- expose why it is deferred
- avoid competing with play sessions for peak resources

"Runs in the background" is not an acceptable performance strategy by itself.

## Attack 131 — Full indexing before first value is a product regression

### Evidence

Windows itself separates indexing completeness from user availability and throttles indexing during active use. UEFN creators already pay substantial editor startup/project costs; adding another mandatory deep-index barrier would directly contradict RELAY's golden-path goal.

### Verdict

NEW PROGRESSIVE-READINESS REQUIREMENT.

### Required changes

Startup/project load should expose stages:

- RELAY host ready
- project metadata ready
- basic deterministic audit ready
- deep/content index ready
- optional semantic/vector index ready

Normal first use should not wait for optional deep/semantic indexing.

## Attack 132 — Polling entire project trees wastes I/O

### Evidence

Microsoft states that the NTFS USN change journal is much more efficient for determining file modifications than repeatedly checking timestamps or registering for file notifications. Earlier RELAY attacks already established that journals/watchers are not authoritative forever because continuity can be lost.

### Verdict

COMBINE JOURNAL-ASSISTED INCREMENTAL WORK WITH RECONCILIATION.

### Required changes

- establish a baseline/reconciliation scan
- use the cheapest trustworthy change feed available
- evaluate NTFS USN journal support on Windows
- target only changed/relevant content
- fall back to reconciliation after continuity gaps
- avoid repeated full-tree polling as the default

## Attack 133 — Every added project cannot become another resident service stack

### Problem

Multi-project support can quietly multiply watchers, parsers, semantic indexes, adapter workers, telemetry, and maintenance work even when only one project is active.

### Verdict

NEW NEAR-ZERO-IDLE-PROJECT REQUIREMENT.

### Required changes

Inactive projects should prefer:

- persisted cold state
- lightweight change tracking
- no active adapter worker unless needed
- no loaded local model/vector index unless queried
- deferred deep reconciliation
- bounded memory/CPU footprint

Measure idle cost per added project.

## Attack 134 — Local model VRAM can be more expensive than remote tokens

### Evidence

Nixie (OSDI 2026) observes that consumer ML model working sets can nearly fill GPU memory and that concurrent applications can cause memory thrashing and CPU-pinned-memory pressure. This is directly relevant to a creator PC sharing one consumer GPU among UEFN, Fortnite, Blender/Krita, and optional local AI.

### Verdict

MODIFY LOCAL-AI ROUTING.

### Required changes

Before local GPU inference:

- inspect/estimate VRAM and GPU headroom
- consider active play/editor workload
- queue or choose another route when contention is material
- benchmark smaller-model/CPU/local/cloud alternatives
- measure foreground performance, not only inference throughput

A local model is not "free" when it causes editor stutter or OOM.

## Attack 135 — Throughput benchmarks can miss the thing users feel

### Evidence

OSDI 1996 work on interactive-system performance argued that throughput-focused benchmarks are inadequate for interactive workloads and demonstrated direct event-latency measurement. More recent systems work continues to focus on tail latency under contention.

### Verdict

NEW INTERACTIVE-LATENCY METRICS.

### Required changes

Performance testing should include:

- p50/p95/p99 command latency
- UEFN/editor responsiveness impact
- play-session frame-time/jank impact where measurable
- project-switch/warm latency
- worst-case maintenance pauses

Average throughput alone cannot approve a RELAY background workload.

## Attack 136 — Database maintenance can create surprise latency and disk amplification

### Evidence

SQLite WAL documentation notes that read performance degrades as WAL grows, checkpoints can produce occasional slower commits, and checkpoint starvation can allow WAL growth. SQLite VACUUM rebuilds the database and may require as much as roughly twice the database size in free disk space during the operation.

### Verdict

NEW STORAGE-MAINTENANCE BUDGET.

### Required changes

Whatever database RELAY selects must benchmark:

- checkpoint/compaction/vacuum latency
- required temporary disk headroom
- foreground read/write impact
- long-reader/long-write behavior
- maintenance interruption/recovery

Maintenance should be scheduled/deferred around active creative work.

## Attack 137 — "SQLite can grow to terabytes" is not a product requirement

### Evidence

SQLite's own limits documentation describes enormous theoretical database limits while explicitly supporting lower runtime limits to prevent excess resource utilization.

### Verdict

SET PRODUCT-LEVEL BOUNDS BELOW TECHNOLOGY MAXIMA.

### Required changes

- project/global storage quotas
- evidence/index/database growth alarms
- application-level maximums appropriate to public hardware
- clear SQLITE_FULL/equivalent degraded behavior if the chosen store reaches a bound
- stress tests at RELAY-supported sizes rather than database theoretical maxima

## Attack 138 — Evidence binaries can dominate operational storage

### Problem

Screenshots, visual regressions, profiler captures, telemetry dumps, and diagnostic artifacts are much larger than normalized metadata.

### Verdict

NEW TIERED-EVIDENCE-STORAGE REQUIREMENT.

### Required changes

Benchmark:

- large BLOBs in database
- files/object-style local storage plus metadata references
- compression
- thumbnails
- content-hash deduplication
- changed-region/delta strategies where appropriate

Keep full evidence only according to retention/pin policy. Metadata and summaries may outlive heavyweight binaries.

## Attack 139 — Resource controls should use the operating system before RELAY reinvents scheduling

### Evidence

Windows exposes EcoQoS/power throttling for non-foreground work and Job Objects for CPU, memory, process, and limit notifications.

### Verdict

BENCHMARK OS-NATIVE RESOURCE CONTROLS FIRST.

### Required changes

Phase 1 should evaluate:

- EcoQoS
- process/thread priority
- memory priority
- Job Object CPU/memory notifications/limits

Prefer soft prioritization/backoff before hard caps. A hard cap that makes jobs fail or creates tail latency is not automatically an improvement.

## Attack 140 — Inactive polling creates invisible power and battery tax

### Evidence

Windows Search uses activity-aware backoff and idle detection rather than indexing at full speed continuously. Older TCP Nice research similarly demonstrated that aggressive background work can harm demand performance while background-aware scheduling can use spare capacity with little foreground interference.

### Verdict

NEW EVENT-DRIVEN/ADAPTIVE-POLLING RULE.

### Required changes

- prefer change events/journals to periodic scans
- back off polling when stable
- coalesce health checks
- suspend inactive integration polling where safe
- raise frequency only during active jobs
- measure background wakeups/CPU/network as part of idle cost

## Attack 141 — Performance profiles should initially be policy, not another wall of settings

### Problem

We need foreground-safe, balanced, idle/batch, and diagnostic behavior, but exposing dozens of CPU/GPU/indexing controls would violate the simplicity attack.

### Verdict

AUTOMATE RESOURCE MODES; EXPOSE ADVANCED OVERRIDES LATER.

### Required changes

Internally support modes such as:

- foreground-safe
- balanced
- idle-boost
- temporary diagnostic burst

Use measured machine/user activity to switch when reliable.

Show the current resource mode in diagnostics, but avoid requiring normal users to tune scheduler knobs.

## Attack 142 — Resource testing on one high-end developer PC proves little

### Evidence

Epic's current UEFN requirements span 16 GB minimum versus 32 GB recommended RAM and 4 GB versus 8 GB recommended VRAM, with NVMe recommended. That is already a materially different resource envelope before considering creator hardware above/below those points.

### Verdict

NEW HARDWARE-TIER BENCHMARK REQUIREMENT.

### Required changes

Benchmark at least:

- UEFN minimum-class hardware
- UEFN recommended-class hardware
- high-end creator workstation

Use versioned hardware fixtures. A feature that is invisible on a high-end workstation but cripples minimum/recommended hardware does not pass.

## Attack 143 — Resource use must be scoped to active work

### Problem

Deep scans, full tests, compaction, historical compatibility scans, and semantic indexing are valuable but not equally urgent.

### Verdict

NEW RESOURCE-AWARE JOB PRIORITY.

### Required changes

Jobs declare:

- foreground/interactive versus background
- expected CPU/RAM/GPU/I/O intensity
- deadline/urgency
- interruptibility/resumability
- active project association

The scheduler may defer lower-value work rather than letting every job compete equally.

## Attack 144 — Resource telemetry can become another observability tax

### Problem

Measuring CPU/GPU/disk/power at high frequency can itself create overhead and data volume.

### Verdict

KEEP PERFORMANCE TELEMETRY TIERED.

### Required changes

- low-rate summary in normal mode
- higher-resolution sampling only for diagnostics/benchmarks
- avoid retaining high-frequency raw counters indefinitely
- benchmark the monitoring overhead itself
- use OS/platform counters rather than custom tracing when they are sufficient

## Attack 145 — Energy cost matters, but data-center totals do not answer desktop scheduling

### Evidence

The Lawrence Berkeley National Laboratory's 2025 update projects U.S. data centers could consume 9.5–15.3% of U.S. electricity by 2030, and DOE explicitly highlights operational/energy-management efficiency. Software-engineering research likewise identifies energy as a growing engineering concern.

### Verdict

MEASURE ENERGY, DO NOT OVERGENERALIZE IT.

### Required changes

- include power/battery/thermal impact in controlled RELAY benchmarks where practical
- do not infer local-versus-cloud energy superiority from national data-center totals
- treat energy as one routing/resource factor alongside privacy, latency, cost, and quality
- report measured local values separately from external/provider estimates

## Attack 146 — Performance regression can erase RELAY's token savings

### Problem

A release that saves 30% AI tokens but adds 15 seconds to project startup, 4 GB idle RAM, or persistent UEFN stutter has failed the larger cost mission.

### Verdict

NEW PERFORMANCE RELEASE GATE.

### Required changes

Track regression budgets for:

- cold/warm startup
- first useful audit
- idle CPU/RAM
- project-switch latency
- indexing cost
- storage growth
- foreground interference
- maintenance pauses
- local-AI coexistence

A milestone can fail for resource regression even if functional and token benchmarks improve.

## Attack 147 — "Capture everything now, optimize later" is incompatible with public use

### Problem

Telemetry, captures, vector indexes, compatibility history, backups, and multi-project evidence all compound over time.

### Verdict

NEW LONG-RUN SOAK REQUIREMENT.

### Required changes

Run multi-day/multi-week accelerated soak tests that measure:

- database/index growth
- evidence storage growth
- memory leaks
- background CPU
- watcher/queue growth
- stale project processes
- compaction/maintenance frequency
- performance after long histories

Public performance testing must include aging, not just clean-install benchmarks.

## Phase 0 performance/resource closure requirements

Phase 0 now also requires:

97. foreground creative workload has priority over optional RELAY/local-AI work
98. background jobs have adaptive backoff/resource semantics
99. basic useful project readiness does not require full deep/semantic indexing
100. Windows indexing evaluates USN/change-feed acceleration plus reconciliation rather than full polling
101. inactive-project routine resource cost is explicitly budgeted
102. local AI routing accounts for GPU/VRAM/foreground contention
103. interactive/tail latency is part of performance acceptance
104. database/index maintenance and temporary disk amplification are benchmarked
105. evidence storage uses quotas/tiering/dedup strategies rather than unbounded binaries
106. Windows-native QoS/resource controls are benchmarked before custom scheduling
107. resource modes do not become mandatory tuning complexity
108. hardware-tier benchmarks include UEFN minimum/recommended/high-end classes
109. resource-intensive jobs declare priority/intensity/interruptibility
110. performance telemetry has its own overhead budget
111. energy is measured where practical without unsupported local-vs-cloud claims
112. release gates include local resource regressions as well as AI-token savings
113. long-run soak tests cover storage/index/resource aging
