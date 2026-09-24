# RELAY Research Plan

## Purpose

RELAY should be based on measured behavior rather than assumptions about AI context, MCP, editor automation, or runtime integrations.

Research outputs must feed specific architecture decisions and benchmarks.

## Research track A — Long context and context compilation

Questions:

- How much context is actually useful for debugging/build tasks?
- How often does full history hurt retrieval or decision quality?
- Which facts must remain exact?
- Which summaries can be lossy?
- Does hierarchical retrieval outperform flat retrieval for project state?
- How should context budgets change across small and large models?
- How do task changes affect prior compacted context?

Initial academic references:

1. Liu et al., "Lost in the Middle: How Language Models Use Long Contexts"
   - https://arxiv.org/abs/2307.03172
   - Relevant to position sensitivity and the danger of assuming that fitting information into a context window means the model will use it reliably.

2. Packer et al., "MemGPT: Towards LLMs as Operating Systems"
   - https://arxiv.org/abs/2310.08560
   - Relevant to tiered/virtual context management and external memory.

3. Sarthi et al., "RAPTOR: Recursive Abstractive Processing for Tree-Organized Retrieval"
   - https://arxiv.org/abs/2401.18059
   - Relevant to hierarchical summaries and retrieval at multiple abstraction levels.

4. Jiang et al., "LLMLingua: Compressing Prompts for Accelerated Inference of Large Language Models"
   - https://aclanthology.org/2023.emnlp-main.825/
   - Relevant to prompt compression and explicit token budgets.

5. Jiang et al., "LongLLMLingua: Accelerating and Enhancing LLMs in Long Context Scenarios via Prompt Compression"
   - https://aclanthology.org/2024.acl-long.91/
   - Relevant to long-context compression, information density, cost, and position effects.

6. Li et al., "Compressing Context to Enhance Inference Efficiency of Large Language Models"
   - https://aclanthology.org/2023.emnlp-main.391/
   - Relevant to selective context compression.

These references motivate experiments; they do not automatically dictate RELAY's implementation.

Additional 2025–2026 evidence:

7. Łajewska et al., "Understanding and Improving Information Preservation in Prompt Compression for LLMs"
   - https://aclanthology.org/2025.findings-emnlp.949/
   - Compression must be evaluated for grounding and information preservation, not token ratio alone.

8. Liu et al., "Context as a Tool: Context Management for Long-Horizon SWE-Agents"
   - https://aclanthology.org/2026.findings-acl.1032/
   - Supports structured active context management instead of append-only/passive histories.

9. Cognitive Scaffold: From Fluid Context to Crystallized Memory for Long-Horizon DeepResearch Agents
   - https://aclanthology.org/2026.acl-long.1170/
   - Supports separating working context from persistent structured memory and protecting atomic numerical/entity facts.

10. Kang et al., "ACON: Optimizing Context Compression for Long-horizon LLM Agents"
   - https://www.microsoft.com/en-us/research/publication/acon-optimizing-context-compression-for-long-horizon-llm-agents/
   - Supports benchmarking smaller/cheaper compression components.

11. Xiong et al., "How Memory Management Impacts LLM Agents: An Empirical Study of Experience-Following Behavior"
   - https://aclanthology.org/2026.acl-long.27/
   - Adds memory-poisoning/error-propagation and misaligned-experience risks.

12. Yang et al., "Grounding Agent Memory in Contextual Intent"
   - https://aclanthology.org/2026.findings-acl.584/
   - Supports intent-aware retrieval rather than semantic similarity alone.

13. Zhang et al., "Lightweight LLM Agent Memory with Small Language Models"
   - https://aclanthology.org/2026.acl-long.588/
   - Supports testing small-model memory retrieval/writing/consolidation under bounded compute.

## Context Gauntlet

Create repeatable tasks and compare:

- full conversation/history
- recent window only
- simple summary
- flat retrieval
- hierarchical retrieval
- RELAY Context Compiler

Test scenarios:

- key fact at start/middle/end
- stale old state versus current state
- exact coordinates/IDs mixed into prose
- 500+ duplicate warnings
- large logs with a tiny relevant incident
- multi-hop dependency question
- task focus changes halfway through
- results spanning files + runtime + asset state

Models:

- small/free-tier class where testable
- mid-tier model
- high-capability model

Metrics:

- task success
- exact fact retention
- wrong-state decisions
- hallucination
- retrieval recall/precision
- total input/output tokens
- tool calls
- latency
- monetary cost where available

## Research track B — CLI/skills versus MCP

Design hypothesis:

CLI + small task skills will often use less context and fewer calls than a large static MCP catalog. This is a hypothesis, not a settled universal rule.

Counter-evidence/current evidence:

- https://proceedings.neurips.cc/paper_files/paper/2024/hash/5a7c947568c1b1328ccc5230172e1e7c-Abstract-Conference.html — SWE-agent; agent-computer interface design materially affects software-agent performance.
- https://arxiv.org/abs/2605.24660 — adaptive tool shortlist depth can outperform fixed exposure strategies on the authors' benchmarks.
- https://arxiv.org/abs/2602.14878 — MCP Tool Descriptions Are Smelly; description quality and compactness materially affect success/cost.
- https://arxiv.org/abs/2602.18914 — description accuracy/functionality affect tool selection.
- https://arxiv.org/abs/2603.20313 — selective semantic tool discovery reports large schema-token savings on its benchmark.
- https://arxiv.org/abs/2608.23992 — production/preprint report of search/execute meta-tools at enterprise scale.

Research question:

Can a thin dynamically discovered MCP surface match or beat CLI+skill on success, context overhead, latency, and recovery for specific client classes?

Test the same workflows using:

1. CLI + compact skill
2. thin MCP with structured execute/result
3. broader domain MCP
4. one-tool-per-operation MCP where available

Measure:

- tool/schema tokens loaded
- execution calls
- retries
- task success
- latency
- model tokens
- ease of recovery
- maintenance complexity

Reddit discussion that triggered this research:
- https://www.reddit.com/r/aigamedev/comments/1v484mi/mcp_is_worse_than_no_mcp_godot/

Treat community posts as hypotheses and experience reports, not authoritative evidence.

## Research track C — UEFN control surfaces

Validate against current official Epic documentation and actual target versions:

- supported UEFN editor automation/control interfaces
- Verse logging/runtime instrumentation
- debug drawing
- play-session control
- screenshot/capture options
- device/entity inspection
- performance/memory interfaces
- allowed runtime networking/instrumentation boundaries
- import/export constraints
- public-release restrictions/validation

Produce a capability matrix by UEFN version.

Do not design core contracts around an undocumented behavior without a fallback.

## Research track D — Runtime observability

Prototype:

- structured log protocol
- probes
- assertions
- stable object references
- runtime event deduplication
- test-state workflows
- fixed captures
- state verification

Measure the smallest telemetry set that reliably answers debugging questions.

## Research track E — Local storage and indexing

Compare candidate storage/index designs for:

- multi-project isolation
- structured state
- append-only history
- result/evidence metadata
- full-text/log search
- dependency graph
- incremental indexing
- migrations
- corruption recovery
- public packaging

Do not select a database purely because it is familiar.

## Research track F — Dashboard usability

Test with users who do not know MCP.

Questions:

- Can they tell whether RELAY is healthy?
- Can they tell what the AI changed?
- Can they find a failed test?
- Can they approve/reject a write safely?
- Can they understand why an integration is unavailable?
- Can they create a diagnostic bundle?

Primary language should remain simple; advanced technical data must remain accessible.

## Research track G — Cost accounting

Define how RELAY estimates and reports savings without fake precision.

Possible metrics:

- local deterministic operations
- bytes/tokens avoided from raw result
- context returned
- cache hit rate
- incremental versus full work
- AI/model calls
- remote calls
- affected tests versus full suite
- elapsed time

"Estimated tokens avoided" must clearly be labeled as an estimate unless measured from an actual alternative run.

## Research track H — Security

Security research starts before remote/public release because project content and local agent execution affect the architecture from the beginning.

Government/current sources:

- NIST/CAISI large-scale agent hijacking red-team (2026):
  https://www.nist.gov/blogs/caisi-research-blog/insights-ai-agent-security-large-scale-red-teaming-competition
- NIST NCCoE Software and AI Agent Identity and Authorization concept paper (2026):
  https://csrc.nist.gov/pubs/other/2026/02/05/accelerating-the-adoption-of-software-and-ai-agent/ipd
- NIST agent identity guidance (2026):
  https://www.nist.gov/blogs/cybersecurity-insights/back-future-why-agentic-ai-needs-strong-identity-foundation
- NIST AI Agent Standards Initiative:
  https://www.nist.gov/news-events/news/2026/02/announcing-ai-agent-standards-initiative-interoperable-and-secure
- NSA/CISA/FBI and partners, Deploying AI Systems Securely:
  https://www.nsa.gov/Press-Room/Press-Releases-Statements/Press-Release-View/Article/3741371/nsa-publishes-guidance-for-strengthening-ai-system-security/
- NSA/CISA/FBI and partners, AI Data Security:
  https://www.nsa.gov/Press-Room/Press-Releases-Statements/Press-Release-View/Article/4192332/nsas-aisc-releases-joint-guidance-on-the-risks-and-best-practices-in-ai-data-se/
- CISA/NCSC/NSA and partners, Guidelines for Secure AI System Development:
  https://www.cisa.gov/news-events/alerts/2023/11/26/cisa-and-uk-ncsc-unveil-joint-guidelines-secure-ai-system-development

Required threat areas:

- indirect prompt injection in project files, docs, logs, repositories, web-derived content, assets, and tool output
- instruction/data boundary failure
- memory poisoning and stale/hostile derived memories
- structured command injection
- shell/path injection
- malicious project content
- secret leakage in logs/context
- cross-project isolation
- remote replay
- approval bypass
- shared user credentials
- long-lived/static credentials
- overbroad local-agent authority
- update supply chain
- diagnostic bundle leakage
- audit/non-repudiation gaps

Before remote/public use:

- threat model
- structured command injection
- shell/path injection
- malicious project content
- secret leakage in logs/context
- cross-project isolation
- remote replay
- approval bypass
- update supply chain
- diagnostic bundle leakage


## Research track I — Human factors and automation

RELAY is an automation system; older human-factors findings are relevant even though the domain differs.

Core sources:

- Bainbridge, "Ironies of Automation" (1983):
  https://doi.org/10.1016/0005-1098(83)90046-8
- NASA, "Human factors of the high technology cockpit" (1990):
  https://ntrs.nasa.gov/citations/19910001630
- NASA, "Potential benefits and hazards of increased reliance on cockpit automation" (1990):
  https://ntrs.nasa.gov/citations/19920056683
- Endsley & Kiris, "The Out-of-the-Loop Performance Problem and Level of Control in Automation" (1995):
  https://doi.org/10.1518/001872095779064555
- Parasuraman & Riley, "Humans and Automation: Use, Misuse, Disuse, Abuse" (1997):
  https://doi.org/10.1518/001872097778543886
- NASA, "Analysis of Autopilot Behavior" (1998):
  https://ntrs.nasa.gov/citations/20020066672

Questions:

- Does RELAY keep users aware of what automation is doing?
- Do automatic repairs or queued jobs create automation surprise?
- Does the user understand the current automation mode and authority?
- Do warnings create cry-wolf/disuse behavior?
- Do approvals create consent fatigue?
- Can users recover when automation fails?
- Does automation hide skills/state the user later needs during an exception?

Evaluation should include user comprehension and recovery, not only task completion.

## Research track J — Government evaluation/accountability guidance

Use government evaluation work as a methodology source, not as automatic product requirements.

Sources:

- NIST AI 800-2, Practices for Automated Benchmark Evaluations of Language Models:
  https://nvlpubs.nist.gov/nistpubs/ai/NIST.AI.800-2.ipd.pdf
- U.S. GAO AI Accountability Framework:
  https://www.gao.gov/products/gao-21-519sp

Benchmark requirements derived for RELAY:

- define the evaluation objective before running
- define what construct is being measured
- state intended use of the result
- choose/document relevant baselines
- pin model/client/tool/interface versions
- document cost controls and protocol settings
- report run-to-run variation/uncertainty where applicable
- retain enough artifacts for reproducibility without violating privacy/security
- distinguish benchmark performance from field behavior
- use field testing/red teaming/post-deployment monitoring where automated benchmarks are insufficient

## Research track K — Native platform versus RELAY duplication

For each engine/tool integration, inventory authoritative native capabilities before implementing a RELAY version.

UEFN/Unreal sources begin with:

- Unreal MCP:
  https://dev.epicgames.com/documentation/unreal-engine/unreal-mcp-in-unreal-editor
- Fortnite 42.00 release notes:
  https://dev.epicgames.com/documentation/fortnite/42-00-fortnite-ecosystem-updates-and-release-notes

Questions:

- Is the native feature authoritative?
- Can RELAY normalize/aggregate it instead of reproducing it?
- What value does RELAY add: cross-tool correlation, history, context compilation, automation, testing, or UX?
- How volatile/version-specific is the native API?
- Is there a safe fallback?

## Benchmark protocol contract

Before a benchmark is used to set a product default, record:

- decision being informed
- hypothesis
- measurement construct
- workload/task set
- baseline(s)
- exact model/client/tool versions
- hardware/runtime where relevant
- protocol/settings
- number of runs
- pass/fail and secondary metrics
- cost/token measurement method
- uncertainty/variation
- known limitations and external-validity limits

A single successful demo is not a benchmark.


## Research output rule

A research task is complete only when it produces one of:

- architecture decision
- rejected option with evidence
- benchmark result
- capability matrix
- implementation requirement
- new unresolved question

Research should not become a pile of links disconnected from product decisions.


## Research track O — Local, cloud, and hybrid execution economics

Question: which work is genuinely cheaper and better to keep local?

Separate deterministic computation from model inference.

For model inference, compare:

- local-only model
- cloud-only model
- local-first with cloud escalation
- task-aware hybrid routing

Measure:

- task success/quality
- end-to-end latency
- model/API cost
- local CPU/GPU time
- power/energy where practical
- hardware utilization
- context/network transfer
- privacy exposure class
- offline availability

Do not generalize mobile/edge results directly to desktop game-development workloads.

Relevant evidence includes 2025–2026 edge/cloud routing and energy studies plus DOE/LBNL data-center energy reports.

## Research track P — File-change continuity and index recovery

File notifications are not assumed complete.

Prototype and test:

- Windows directory-change notifications under bursty edits
- overflow/error recovery
- NTFS change-journal acceleration
- journal discontinuity recovery
- startup reconciliation
- periodic reconciliation
- rename/move/delete storms
- large generated asset imports
- crash between change detection and durable index commit

Success requires a provable route back to correct project state after missed events.

## Research track Q — Storage durability and rebuildability

Compare candidate local stores for:

- crash recovery
- integrity checking
- backup/snapshot support
- migration safety
- concurrent readers/writers
- corruption detection
- large result/evidence metadata
- rebuild time
- storage quotas
- operation on local versus synced/network-backed project paths

Design principle to validate: portable project configuration may live with the project while mutable operational state lives in a local RELAY data area.

## Research track R — Windows local-host process model

Compare:

- normal per-user background process
- Windows per-user service mechanisms
- traditional system service plus per-user companion
- minimal optional privileged helper patterns

Evaluate:

- ability to integrate with UEFN/Krita/Blender in the interactive user session
- least privilege
- startup/login/logout behavior
- multi-user behavior
- update/install needs
- IPC complexity
- recovery after crash/restart
- public installer complexity

Do not select a Session-0 service model by default.

## Research track S — Local IPC and release/update integrity

Local IPC evaluation must cover:

- peer identity
- operating-system access rules
- protocol versioning
- cross-user access tests
- restart/reconnect behavior
- dashboard-to-host authentication assumptions

Release/update evaluation must cover:

- signed/verifiable artifacts
- update metadata integrity
- rollback/recovery
- dependency inventory
- build provenance/attestation options
- interrupted update testing
- separation between normal runtime and update privileges


## Evidence inputs for tracks O–S

Local/cloud economics:

- Guégain & Coignion, "The Battery Price of edge AI" (2026): https://arxiv.org/abs/2609.11940
- Arya & Simmhan, "Understanding the Performance and Power of LLM Inferencing on Edge Accelerators" (2025): https://arxiv.org/abs/2506.09554
- Yu, Goudarzi & Toosi, "Efficient Routing of Inference Requests across LLM Instances in Cloud-Edge Computing" (2025/2026): https://arxiv.org/abs/2507.15553
- Lawrence Berkeley National Laboratory, United States Data Center Energy Usage Report: 2025 Update: https://datacenters.lbl.gov/publications/united-states-data-center-energy-2025

Windows file/index continuity:

- Microsoft ReadDirectoryChangesW: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw
- Microsoft Change Journal Records: https://learn.microsoft.com/en-us/windows/win32/fileio/change-journal-records
- Microsoft USN_JOURNAL_DATA_V2: https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ns-winioctl-usn_journal_data_v2

Storage durability candidates:

- SQLite WAL: https://www.sqlite.org/wal.html
- SQLite Backup API: https://sqlite.org/backup.html
- SQLite corruption/recovery notes: https://www.sqlite.org/howtocorrupt.html
- SQLite PRAGMA integrity checks: https://sqlite.org/pragma.html

Windows local runtime:

- Microsoft Interactive Services: https://learn.microsoft.com/en-us/windows/win32/services/interactive-services
- Microsoft Per-user services: https://learn.microsoft.com/en-us/windows/application-management/per-user-services-in-windows
- Microsoft AppContainer isolation: https://learn.microsoft.com/en-us/windows/win32/secauthz/appcontainer-isolation
- Microsoft named-pipe security: https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights

Secure delivery:

- NIST SP 800-218 Rev. 1 initial public draft: https://csrc.nist.gov/pubs/sp/800/218/r1/ipd
- CISA/FBI Product Security Bad Practices update (2025): https://www.cisa.gov/news-events/alerts/2025/01/17/cisa-and-fbi-release-updated-guidance-product-security-bad-practices
- SLSA build provenance: https://slsa.dev/spec/v1.2/build-provenance


## Research track T — Third-party adapter isolation and marketplace risk

Questions:

- Can adapters be useful while remaining out of process?
- What Windows isolation mechanism gives the best balance of compatibility and least privilege?
- Which capabilities need brokered access versus direct OS access?
- Can first-party adapters use the same manifest/broker model without unacceptable overhead?
- What should an early public extension story look like before any open marketplace exists?

Threat cases to include:

- overbroad filesystem access
- unnecessary network access
- publisher/account compromise
- unsafe update
- dependency compromise
- abandoned/taken-over package
- misleading command/tool metadata
- excessive background resource use
- incompatible adapter version
- adapter crash/hang
- cross-project access

Prototype candidates:

- one worker process per adapter
- adapter pools separated by trust class
- OS-level sandbox/restricted execution options available on supported Windows versions
- brokered file/tool/network capabilities
- explicit adapter capability manifest
- local-only manual install versus curated catalog

Measure:

- task success
- startup/command latency
- memory/CPU overhead
- failure isolation
- compatibility with UEFN/Blender/Krita
- ease of permission explanation
- cross-project isolation
- recoverability after worker failure

## Research track U — Adapter provenance, components, and updates

Define a machine-readable adapter record containing:

- publisher identity
- exact artifact/version/digest
- RELAY protocol compatibility
- supported external-tool versions
- declared capabilities
- dependency/component inventory
- build/release provenance when available
- update source/channel
- review/curation status

Evaluate:

- SBOM formats and practical granularity
- artifact signing/verification
- provenance/attestation approaches
- exact-version pinning
- dependency locking
- vulnerability monitoring
- update staging/rollback
- handling of publisher or update-channel compromise

Do not use "signed" or "verified" as a synonym for safe.

## Research track V — Extension cost and context containment

Test whether adding adapters recreates the tool/context bloat RELAY is intended to eliminate.

Scenarios:

- 1, 10, 50, and 200 installed adapter command sets
- all commands exposed
- project-filtered commands
- task-filtered dynamic discovery
- disabled/inactive adapters
- adapter health telemetry at different polling rates

Measure:

- tool/schema tokens
- model selection accuracy
- latency
- AI calls
- daemon CPU/memory
- background I/O
- dashboard clarity

The public extension architecture is acceptable only if inactive/unrelated adapters have negligible effect on AI context and ordinary runtime cost.


## Research track W — Data classification and egress policy

Questions:

- What is the smallest usable sensitivity taxonomy for public creators?
- Can classification remain understandable without enterprise-policy jargon?
- How should path rules, content rules, user labels, and project defaults interact?
- How should derived artifacts inherit or lower sensitivity?
- What should happen when provider retention/training/data-use information is unknown or stale?

Prototype:

- project sensitivity defaults
- per-path/per-artifact overrides
- destination/provider policy profiles
- task-scoped egress decisions
- outbound data lineage/ledger
- conservative behavior for unknown provider policy

Tests:

- mixed public/private files
- proprietary code with no detectable secret pattern
- prohibited remote destination
- provider policy becomes stale
- cross-project data request
- user override and policy conflict

## Research track X — Credential broker and secret containment

Compare secure local secret stores and broker designs suitable for public Windows use.

Requirements to test:

- opaque credential handles exposed to AI
- late secret resolution
- narrow subprocess credential injection
- no secret values in normal logs/results
- rotation/revocation
- adapter-specific credential scopes
- crash/restart behavior
- redaction failure cases

Secret scanners remain a secondary detection control, not the architecture.

## Research track Y — Embedding/vector privacy

Evaluate:

- local versus remote embedding generation
- retrieval quality without remote embeddings
- vector-store isolation
- sensitivity propagation
- deletion/rebuild behavior
- privacy risks from embedding inversion
- whether quantization/noise or other defenses are appropriate for RELAY tasks

Academic baselines include:

- Morris et al., EMNLP 2023, Text Embeddings Reveal (Almost) As Much As Text
- Huang et al., ACL 2024, Transferable Embedding Inversion Attack
- Chen et al., ACL 2025, ALGEN

Do not market embeddings as anonymization.

## Research track Z — Multimodal privacy and prompt-injection boundary

Test realistic project captures containing:

- source code
- usernames/paths
- private chat windows
- unreleased assets
- credential-like text
- benign and adversarial instruction-like visual content

Compare:

- full-desktop capture
- application-window capture
- fixed project viewport
- local crop/redaction
- metadata-only visual diff
- remote multimodal analysis

Measure:

- debugging usefulness
- accidental disclosure
- prompt-injection susceptibility
- tokens/bytes transferred
- user comprehension of what was sent

## Research track AA — Local-only/private mode verification

Define a testable network contract rather than a UI label.

Black-box tests should observe whether project-derived data leaves through:

- model APIs
- embedding APIs
- remote gateway
- analytics
- crash/support upload
- adapters
- diagnostics

Update checks and other product networking must be documented separately.

Success criterion: RELAY can demonstrate the difference between "no project-data egress" and "fully offline" rather than conflating the two.

## Research track AB — Provider-policy drift

Remote service policy is external, versioned state.

Research:

- machine-readable provider policy metadata where available
- manual/curated fallback
- policy source and verification timestamps
- account/tier/workspace differences
- data residency attributes
- retention/training/data-use changes over time
- behavior when policy knowledge is incomplete

RELAY should prefer an explicit "unknown" state over stale reassurance.


## Research track AC — Collaborative state freshness

Test shared-project scenarios where humans and AI clients change the same project over time.

Cases:

- plan created at project revision R1 and executed after R2
- two agents change the same file or entity
- two agents make independent changes that share a dependency
- external editor state changes after RELAY inspection
- long-running jobs cross version/integration changes

Compare revision/hash preconditions, optimistic conflict detection, resource claims, serialized writes, and merge/reconcile workflows.

Measure silent conflicts, successful recovery, added latency, replanning work, model/tool calls, and user interruptions.

SyncMind (ICML 2025) is a primary academic baseline.


## Research track AD — Workspace roles and delegated identity

Compare simple role templates, role-based policy with constraints, role plus resource/attribute policy, and external provider permissions combined with stricter RELAY project policy.

Test multiple humans, outside collaborators, one user in multiple workspaces, personal versus shared connections, one user using multiple AI clients, and agent-to-sub-agent delegation.

Measure policy correctness, administrative complexity, user comprehension, over-granting, and audit reconstruction.

Research baselines include NIST RBAC/ABAC work, NIST zero-trust guidance, RFC 8693 delegation semantics, RFC 8707 resource indicators, and RFC 9700 OAuth security BCP.
