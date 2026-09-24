# Phase 0 Evidence Register

## Purpose

This register collects evidence that materially challenges or strengthens RELAY's architecture. Sources are included because they change a requirement, benchmark, or decision—not merely because they mention agents.

Confidence labels:

- High — peer-reviewed/established publication or first-party government/platform source directly relevant to the claim.
- Medium — credible recent preprint or production report that still needs independent replication.
- Historical — older evidence whose technology differs but whose human-factors finding is relevant.

## Long context and compression

### Lost in the Middle: How Language Models Use Long Contexts

- Year: 2024
- Type: peer-reviewed, TACL
- Confidence: High
- URL: https://aclanthology.org/2024.tacl-1.9/
- Finding: relevant information position can significantly affect long-context performance; middle content is often used less reliably.
- RELAY impact: do not equate context-window capacity with usable working memory.

### Understanding and Improving Information Preservation in Prompt Compression for LLMs

- Year: 2025
- Type: Findings of EMNLP
- Confidence: High
- URL: https://aclanthology.org/2025.findings-emnlp.949/
- Finding: some compression methods lose key information; compression should be evaluated for task performance, grounding, and information preservation.
- RELAY impact: protect exact facts and benchmark compression quality, not only token ratio.

### Context as a Tool: Context Management for Long-Horizon SWE-Agents

- Year: 2026
- Type: Findings of ACL
- Confidence: High
- URL: https://aclanthology.org/2026.findings-acl.1032/
- Finding: append-only/passive context management can cause explosion and semantic drift; structured context with stable semantics, condensed memory, and high-fidelity recent history improves long-horizon SWE behavior under bounded context.
- RELAY impact: strengthens active Context Compiler design.

### Cognitive Scaffold: From Fluid Context to Crystallized Memory for Long-Horizon DeepResearch Agents

- Year: 2026
- Type: ACL
- Confidence: High
- URL: https://aclanthology.org/2026.acl-long.1170/
- Finding: factorized working context plus persistent structured memory; explicitly preserves atomic numerical/entity constraints.
- RELAY impact: strengthens exact-field protection and structured memory.

### ACON: Optimizing Context Compression for Long-horizon LLM Agents

- Year: 2026
- Type: ICML / Microsoft Research
- Confidence: High
- URL: https://www.microsoft.com/en-us/research/publication/acon-optimizing-context-compression-for-long-horizon-llm-agents/
- Finding: context compression can materially reduce peak tokens while preserving task performance; smaller compressors can reduce overhead.
- RELAY impact: test small/local components for context maintenance instead of expensive frontier-model calls.

## Memory quality and retrieval

### How Memory Management Impacts LLM Agents: An Empirical Study of Experience-Following Behavior

- Year: 2026
- Type: ACL
- Confidence: High
- URL: https://aclanthology.org/2026.acl-long.27/
- Finding: retrieved experiences can cause strong experience-following; bad or mismatched memories can propagate errors.
- RELAY impact: memory quality, quarantine, provenance, and stale/conflict handling are required.

### Grounding Agent Memory in Contextual Intent

- Year: 2026
- Type: Findings of ACL
- Confidence: High
- URL: https://aclanthology.org/2026.findings-acl.584/
- Finding: semantically similar memories may be wrong under different goals/constraints; intent-aware indexing reduces retrieval noise.
- RELAY impact: context retrieval must include goal/action/entity intent, not similarity alone.

### Lightweight LLM Agent Memory with Small Language Models

- Year: 2026
- Type: ACL
- Confidence: High
- URL: https://aclanthology.org/2026.acl-long.588/
- Finding: memory retrieval/writing/consolidation can be split across tiers and driven efficiently with smaller models.
- RELAY impact: reinforces cheap local/small-model memory operations as a benchmark target.

## Tool interfaces, MCP, and CLI assumptions

### SWE-agent: Agent-Computer Interfaces Enable Automated Software Engineering

- Year: 2024
- Type: NeurIPS peer-reviewed
- Confidence: High
- URL: https://proceedings.neurips.cc/paper_files/paper/2024/hash/5a7c947568c1b1328ccc5230172e1e7c-Abstract-Conference.html
- Finding: specially designed agent-computer interfaces materially affected software-agent performance.
- RELAY impact: the real question is interface quality, not CLI-versus-MCP branding. RELAY needs an agent-oriented interface benchmark.

### How Many Tools Should an LLM Agent See? A Chance-Corrected Answer

- Year: 2026
- Type: preprint
- Confidence: Medium
- URL: https://arxiv.org/abs/2605.24660
- Finding: adaptive shortlists can preserve tool coverage while presenting far fewer tools; showing too many or too few both create failure modes.
- RELAY impact: capability/tool shortlist depth should be adaptive and measured, not a fixed large catalog.

### Model Context Protocol Tool Descriptions Are Smelly!

- Year: 2026
- Type: empirical preprint
- Confidence: Medium
- URL: https://arxiv.org/abs/2602.14878
- Finding: 97.1% of 856 studied tool descriptions had at least one identified smell; better descriptions improved median task success but often increased execution steps; compact variants could retain reliability with lower token overhead.
- RELAY impact: "MCP is bad" is too simple. Interface quality and compactness matter.

### From Docs to Descriptions: Smell-Aware Evaluation of MCP Server Descriptions

- Year: 2026
- Type: empirical preprint
- Confidence: Medium
- URL: https://arxiv.org/abs/2602.18914
- Finding: description quality affects tool selection and reliability at ecosystem scale.
- RELAY impact: command metadata quality is a correctness requirement.

### Semantic Tool Discovery for Large Language Models

- Year: 2026
- Type: preprint
- Confidence: Medium
- URL: https://arxiv.org/abs/2603.20313
- Finding: dynamically selecting a few relevant tools can dramatically reduce schema-token overhead on the authors' benchmark.
- RELAY impact: benchmark dynamic tool discovery against CLI/skills.

### SCOUT / Hybrid Semantic Tool Discovery for Enterprise MCP Gateway

- Year: 2026
- Type: production/preprint report
- Confidence: Medium
- URL: https://arxiv.org/abs/2608.23992
- Finding: reports a production architecture exposing meta-tools for tool search/execution rather than full catalogs, with large tool-token reductions.
- RELAY impact: a thin remote MCP can remain efficient if capabilities are selected dynamically.

## Government: agent security, identity, evaluation

### NIST/CAISI — Insights into AI Agent Security from a Large-Scale Red-Teaming Competition

- Date: 2026-03-23
- Type: U.S. government research summary
- Confidence: High
- URL: https://www.nist.gov/blogs/caisi-research-blog/insights-ai-agent-security-large-scale-red-teaming-competition
- Finding: more than 250,000 attack attempts from over 400 participants against 13 frontier models; at least one successful hijacking attack against every target model.
- RELAY impact: project/tool content is untrusted input; model-only prompt defenses are insufficient.

### NIST NCCoE — Software and AI Agent Identity and Authorization Concept Paper

- Date: 2026-02-05
- Type: U.S. government draft concept paper
- Confidence: High for problem framing; draft status for prescriptions
- URL: https://csrc.nist.gov/pubs/other/2026/02/05/accelerating-the-adoption-of-software-and-ai-agent/ipd
- Finding: calls out identification, authorization, auditing, non-repudiation, and prompt-injection controls for software/AI agents.
- RELAY impact: first-class client/agent identity and delegated authorization must be architecture-level concerns.

### NIST — Back to the Future: Why Agentic AI Needs a Strong Identity Foundation

- Date: 2026-08-27
- Type: U.S. government cybersecurity guidance/blog
- Confidence: High for design guidance
- URL: https://www.nist.gov/blogs/cybersecurity-insights/back-future-why-agentic-ai-needs-strong-identity-foundation
- Finding: warns against shared user credentials, static/long-lived credentials, broad scopes, unsandboxed local-agent access, and excessive human-in-the-loop prompts causing consent fatigue.
- RELAY impact: scoped/revocable authority, agent identity, sandboxing, and risk-adaptive approvals.

### NIST AI 800-2 — Practices for Automated Benchmark Evaluations of Language Models

- Date: 2026-01
- Type: U.S. government initial public draft
- Confidence: High for evaluation methodology; draft status
- URL: https://nvlpubs.nist.gov/nistpubs/ai/NIST.AI.800-2.ipd.pdf
- Finding: evaluation should define objectives/measurement constructs, choose fitting benchmarks/baselines, and report protocol, uncertainty, cost controls, and results sufficiently for valid interpretation/reproducibility.
- RELAY impact: Context Gauntlet and cost comparisons need explicit measurement protocols.

### NIST AI Agent Standards Initiative

- Date: 2026-02-17
- Type: U.S. government initiative
- Confidence: High
- URL: https://www.nist.gov/news-events/news/2026/02/announcing-ai-agent-standards-initiative-interoperable-and-secure
- Finding: agent interoperability, protocol development, security, and identity are active standardization areas.
- RELAY impact: avoid locking core behavior to one current protocol.

### U.S. GAO — Artificial Intelligence: An Accountability Framework

- Date: 2021-06-30
- Type: U.S. government accountability framework
- Confidence: High
- URL: https://www.gao.gov/products/gao-21-519sp
- Finding: emphasizes governance, data, performance, and monitoring across the AI lifecycle and notes that system inputs/operations may not be visible.
- RELAY impact: continuous monitoring, traceability, and auditable operating state support the dashboard/transaction design.

### NSA/CISA/FBI and international partners — Deploying AI Systems Securely

- Date: 2024-04-15
- Type: U.S. government/joint cybersecurity guidance
- Confidence: High
- URL: https://www.nsa.gov/Press-Room/Press-Releases-Statements/Press-Release-View/Article/3741371/nsa-publishes-guidance-for-strengthening-ai-system-security/
- Finding: secure AI deployment requires lifecycle security and resilience rather than only model behavior controls.
- RELAY impact: secure deployment, update, incident response, and recovery are architecture work, not Phase-11 decoration.

### NSA/CISA/FBI and partners — AI Data Security

- Date: 2025-05-22
- Type: U.S. government/joint cybersecurity guidance
- Confidence: High
- URL: https://www.nsa.gov/Press-Room/Press-Releases-Statements/Press-Release-View/Article/4192332/nsas-aisc-releases-joint-guidance-on-the-risks-and-best-practices-in-ai-data-se/
- Finding: emphasizes data provenance, authenticating trusted revisions, supply-chain risk, maliciously modified data, and data drift.
- RELAY impact: strengthens provenance/trust/freshness requirements for project memory and context.

### CISA/NSA/NCSC and partners — Guidelines for Secure AI System Development

- Date: 2023-11
- Type: joint government cybersecurity guidance
- Confidence: High
- URL: https://www.cisa.gov/news-events/alerts/2023/11/26/cisa-and-uk-ncsc-unveil-joint-guidelines-secure-ai-system-development
- Finding: applies Secure by Design across design, development, deployment, and operation.
- RELAY impact: security requirements move earlier in the roadmap; they cannot wait for public-beta hardening.

### U.S. DoD CDAO — Responsible AI Toolkit

- Date: 2023-11-14
- Type: U.S. Department of Defense implementation toolkit
- Confidence: High for process/governance framing
- URL: https://www.defense.gov/News/Releases/Release/Article/3588743/cdao-releases-responsible-ai-rai-toolkit-for-ensuring-alignment-with-rai-best-p/
- Finding: operationalizes responsible-AI principles with technical/process tools and draws on NIST/IEEE work.
- RELAY impact: reinforces lifecycle evaluation, traceability, and documented operating controls.

### DARPA — Air Combat Evolution / human trust in autonomy

- Type: U.S. government R&D program
- Confidence: High for research objective, not a RELAY-specific result
- URL: https://www.darpa.mil/research/programs/air-combat-evolution
- Finding: explicitly treats measurement/calibration of human trust as a research problem in human-machine teaming.
- RELAY impact: dashboard trust/automation behavior should be measured rather than assumed.

## Government and historical human factors

### NASA — Human factors of the high technology cockpit

- Year: 1990
- Type: NASA conference paper
- Confidence: Historical
- URL: https://ntrs.nasa.gov/citations/19910001630
- Finding: reliable automation can reduce physical workload while retaining high cognitive demand and creating new serious human-error/complacency risks.
- RELAY impact: automation does not eliminate the need for situation awareness.

### NASA — Potential benefits and hazards of increased reliance on cockpit automation

- Year: 1990
- Type: NASA-supported conference paper
- Confidence: Historical
- URL: https://ntrs.nasa.gov/citations/19920056683
- Finding: many automation problems arise at the human-automation interface, not from equipment failure.
- RELAY impact: UX/authority visibility and recovery design are safety features.

### NASA — Analysis of Autopilot Behavior

- Year: 1998
- Type: NASA technical-report record
- Confidence: Historical
- URL: https://ntrs.nasa.gov/citations/20020066672
- Finding: "automation surprises" can result from mismatch between operator mental model and actual automated behavior.
- RELAY impact: dashboard must make current automation state, authority, and actual changes legible.

### Bainbridge — Ironies of Automation

- Year: 1983
- Type: classic peer-reviewed automation paper
- Confidence: Historical
- URL: https://doi.org/10.1016/0005-1098(83)90046-8
- Finding: automation can expand rather than eliminate human-operator problems, especially when humans are left to handle abnormal cases.
- RELAY impact: do not automate routine work then leave users blind during rare failures.

### Endsley & Kiris — The Out-of-the-Loop Performance Problem

- Year: 1995
- Type: peer-reviewed human-factors study
- Confidence: Historical
- URL: https://doi.org/10.1518/001872095779064555
- Finding: automation can reduce situation awareness and impair manual takeover after failure.
- RELAY impact: maintain user visibility and meaningful control.

### Parasuraman & Riley — Humans and Automation: Use, Misuse, Disuse, Abuse

- Year: 1997
- Type: peer-reviewed human-factors paper
- Confidence: Historical
- URL: https://doi.org/10.1518/001872097778543886
- Finding: overreliance, underuse due false alarms, and poorly designed automation can all undermine performance.
- RELAY impact: warning quality, false positives, trust calibration, and automation policy matter.

## UEFN / Unreal authoritative platform evidence

### Epic — Unreal MCP in Unreal Editor 5.8

- Year: 2026
- Type: first-party platform documentation
- Confidence: High
- URL: https://dev.epicgames.com/documentation/unreal-engine/unreal-mcp-in-unreal-editor
- Finding: Unreal MCP is Experimental; features are incomplete/missing and APIs/data formats may change.
- RELAY impact: UEFN MCP must be version/capability-gated behind an adapter, never a core dependency.

### Epic — Fortnite 42.00 Ecosystem Updates

- Date: 2026-08-20
- Type: first-party platform release notes
- Confidence: High
- URL: https://dev.epicgames.com/documentation/fortnite/42-00-fortnite-ecosystem-updates-and-release-notes
- Finding: Unreal MCP became available in UEFN.
- RELAY impact: valuable first-party integration path exists, but inherits experimental surface risk.

## Research gaps still open

- independent replication of very recent 2026 MCP/tool-discovery claims
- real token/cost comparison of RELAY CLI+skills versus thin dynamic MCP
- security behavior of different cloud ChatGPT connector paths
- exact UEFN MCP compatibility/version surface in UEFN, not only base Unreal
- practical sandboxing model for local Windows RELAY workers
- retention/privacy requirements for public consumer use
- user studies for approval/alarm fatigue in this exact development workflow
- modality-preserving retrieval for screenshots and visual history


## Adapter and extension ecosystem evidence

### Beyond the Protocol: Unveiling Attack Vectors in the Model Context Protocol Ecosystem

- Year: 2025
- Type: research preprint
- Confidence: Medium
- Source: arXiv 2506.02040
- Finding: researchers demonstrated multiple unsafe-server attack classes and reported insufficient review on several MCP aggregation platforms; a small user study found users struggled to identify unsafe servers.
- RELAY impact: a registry/marketplace cannot be the trust boundary.

### Toward Understanding Security Issues in the Model Context Protocol Ecosystem

- Year: 2025
- Type: research preprint
- Confidence: Medium
- Source: arXiv 2510.16558
- Finding: analysis of 67,057 servers across six public registries identified weak vetting and server-hijack risks.
- RELAY impact: adapter distribution needs artifact identity, publisher/provenance metadata, capability boundaries, and quarantine.

### An Empirical Study of Model Context Protocol Applications

- Year: 2026
- Type: research preprint
- Confidence: Medium
- Source: arXiv 2607.25635
- Finding: among 1,723 studied MCP applications, logging and enable/disable controls were common but only 37.2% used blocking approval before tool execution.
- RELAY impact: oversight semantics vary widely and must be explicit in RELAY rather than inherited from host defaults.

### Developers Are Victims Too: A Comprehensive Analysis of the VS Code Extension Ecosystem

- Year: 2024
- Type: research preprint
- Confidence: Medium
- Source: arXiv 2411.07479
- Finding: analysis of 52,880 extensions found about 5.6% with suspicious behavior and highlighted the power third-party development extensions can gain over developer environments.
- RELAY impact: third-party adapter code should not run unchecked inside the core process.

### CISA vulnerability bulletin covering the 2025 Nx package compromise

- Year: 2025
- Type: U.S. government vulnerability bulletin
- Confidence: High for incident record
- URL: https://www.cisa.gov/news-events/bulletins/sb25-272
- Finding: compromised build-system packages/plugins were distributed through npm.
- RELAY impact: previously trusted packages and update channels can be compromised; signatures/provenance and rollback are necessary but not sufficient by themselves.

### NIST Software Bill of Materials guidance

- Type: U.S. government software supply-chain guidance
- Confidence: High
- URL: https://www.nist.gov/itl/executive-order-14028-improving-nations-cybersecurity/software-supply-chain-security-guidance-20
- Finding: NIST emphasizes machine-readable component inventories, supplier provenance, signatures, and ongoing vulnerability context.
- RELAY impact: adapters should expose dependency/component inventory and provenance suitable for automated assessment.

### NIST IR 8536 — Supply Chain Traceability Principles

- Year: 2026
- Type: U.S. government report
- Confidence: High for traceability principles
- URL: https://csrc.nist.gov/pubs/ir/8536/final
- Finding: verifiable provenance chains, interoperable traceability, cryptographic linkage, and selective disclosure can improve supply-chain assurance.
- RELAY impact: use these as design principles for adapter provenance/history; the report is manufacturing-focused and is not itself a software-plugin standard.


## Data boundary, privacy, and remote-processing evidence

### NSA/CISA/FBI et al. — AI Data Security: Best Practices for Securing Data Used to Train & Operate AI Systems

- Year: 2025
- Type: joint government cybersecurity guidance
- Confidence: High for lifecycle/data-security requirements
- URL: https://www.nsa.gov/Press-Room/Press-Releases-Statements/Press-Release-View/Article/4192332/nsas-aisc-releases-joint-guidance-on-the-risks-and-best-practices-in-ai-data-se/
- Finding: AI-system data is part of the supply chain and should be protected across the lifecycle with provenance and trusted infrastructure.
- RELAY impact: remote AI/embedding/support processing becomes an explicit data-egress boundary with provenance and destination policy.

### NIST Privacy Framework 1.0

- Year: 2020
- Type: U.S. government privacy framework
- Confidence: High for privacy-engineering principles
- URL: https://doi.org/10.6028/NIST.CSWP.01162020
- Finding: privacy management includes lifecycle data processing, selective disclosure, processing permissions, deletion, audit minimization, and limiting observability/linkability.
- RELAY impact: data minimization is a privacy rule as well as a token-cost rule; egress policy must precede context optimization.

### NIST/CAISI — Strengthening AI Agent Hijacking Evaluations

- Year: 2025
- Type: U.S. government technical research blog
- Confidence: High for demonstrated evaluation risks
- URL: https://www.nist.gov/news-events/news/2025/01/technical-blog-strengthening-ai-agent-hijacking-evaluations
- Finding: extended agent-hijacking evaluations included mass data-disclosure tasks and frequently induced agents to follow malicious instructions.
- RELAY impact: read access and external-send authority must be separate capabilities.

### Simple Prompt Injection Attacks Can Leak Personal Data Observed by LLM Agents During Task Execution

- Year: 2025
- Type: research preprint
- Confidence: Medium
- Source: arXiv 2506.01055
- Finding: prompt injection caused tool-calling agents to leak personal data observed during execution; in the extended evaluation no built-in defense fully prevented leakage.
- RELAY impact: data-egress enforcement cannot depend on model refusal behavior.

### Text Embeddings Reveal (Almost) As Much As Text

- Year: 2023
- Type: EMNLP peer-reviewed
- Confidence: High
- URL: https://aclanthology.org/2023.emnlp-main.765/
- Finding: under the paper's tested conditions, substantial portions of short source texts could be reconstructed from dense embeddings, including personal information.
- RELAY impact: embeddings are sensitive derived data, not anonymous by default.

### Transferable Embedding Inversion Attack

- Year: 2024
- Type: ACL peer-reviewed
- Confidence: High
- URL: https://aclanthology.org/2024.acl-long.230/
- Finding: the authors demonstrated transfer-style reconstruction attacks without direct access to the original embedding model.
- RELAY impact: a vector store or remote embedding service inherits source confidentiality requirements.

### ALGEN: Few-shot Inversion Attacks on Textual Embeddings

- Year: 2025
- Type: ACL peer-reviewed
- Confidence: High
- URL: https://aclanthology.org/2025.acl-long.1185/
- Finding: the study reduced data requirements for black-box embedding inversion and recovered key information across domains/languages.
- RELAY impact: derived vector data must remain inside sensitivity/retention policy.

### Manipulating Multimodal Agents via Cross-Modal Prompt Injection

- Year: 2025
- Type: research preprint
- Confidence: Medium
- Source: arXiv 2504.14348
- Finding: adversarial visual/textual content altered multimodal-agent behavior across tested tasks.
- RELAY impact: screenshots are not passive trustworthy evidence; they are both sensitive content and an untrusted instruction surface.

### OWASP GenAI guidance — Prompt Injection, Sensitive Information Disclosure, System Prompt Leakage

- Year: 2025
- Type: industry security guidance
- Confidence: Medium-High for application-security practices
- URLs:
  - https://genai.owasp.org/llmrisk/llm01-prompt-injection/
  - https://genai.owasp.org/llmrisk/llm022025-sensitive-information-disclosure/
  - https://genai.owasp.org/llmrisk/llm072025-system-prompt-leakage/
- Finding: prompt injection can lead to sensitive-data disclosure and connected-system actions; credentials should not be embedded in prompts/system instructions.
- RELAY impact: credentials become opaque capabilities resolved outside model context, and multimodal input remains untrusted.


### CodeCloak: Evaluating and Mitigating Code Leakage by LLM Code Assistants

- Year: 2024
- Type: research preprint
- Confidence: Medium
- Source: arXiv 2404.09066
- Finding: cloud code assistants can receive proprietary repository context, creating a code-disclosure trade-off; the paper evaluates prompt transformations intended to reduce leakage while preserving utility.
- RELAY impact: remote coding/model clients should receive policy-selected minimal context rather than unrestricted project access.

### When GPT Spills the Tea: Knowledge File Leakage in GPTs

- Year: 2025
- Type: ACL peer-reviewed
- Confidence: High
- URL: https://aclanthology.org/2025.acl-long.936/
- Finding: analysis found multiple leakage pathways involving prompts, retrieval, execution environments, and metadata such as file titles/types/sizes.
- RELAY impact: privacy classification must cover metadata and execution-derived artifacts, not only raw file content.


## Collaboration, identity, delegation, and concurrency evidence

### NIST — Accelerating the Adoption of Software and AI Agent Identity and Authorization

- Year: 2026
- Type: U.S. government concept paper
- Confidence: High for problem framing and requirements direction
- URLs:
  - https://www.nist.gov/news-events/news/2026/02/new-concept-paper-identity-and-authority-software-agents
  - https://csrc.nist.gov/pubs/other/2026/02/05/accelerating-the-adoption-of-software-and-ai-agent/ipd
- Finding: NIST explicitly identifies agent identification, authorization, auditing, non-repudiation, and prompt-injection controls as core concerns when agents receive access to data, tools, and applications.
- RELAY impact: distinguish human, client, agent, and service identities and preserve delegated authority in audit records.

### NIST SP 800-162 — Attribute Based Access Control

- Year: 2014/updated 2019
- Type: U.S. government standard guidance
- Confidence: High
- URL: https://www.nist.gov/publications/guide-attribute-based-access-control-abac-definition-and-considerations-0
- Finding: authorization can be determined from subject, object, operation, and environmental attributes against policy.
- RELAY impact: simple role templates can coexist with project/resource/action attributes and contextual policy.

### NIST Role-Based Access Control research and model

- Years: 1992–2000
- Type: U.S. government/academic access-control research
- Confidence: High
- URLs:
  - https://www.nist.gov/publications/role-based-access-controls
  - https://www.nist.gov/publications/nist-model-role-based-access-control-towards-unified-standard
- Finding: roles reduce authorization-management complexity and can include hierarchies and constraints.
- RELAY impact: roles are appropriate for UX/admin templates but should not replace resource-specific constraints.

### NIST SP 800-207 and SP 800-207A — Zero Trust Architecture

- Years: 2020 and 2023
- Type: U.S. government security guidance
- Confidence: High
- URLs:
  - https://csrc.nist.gov/pubs/sp/800/207/final
  - https://csrc.nist.gov/pubs/sp/800/207/a/final
- Finding: network location, affiliation, or ownership should not imply trust; access should be resource-specific, least-privilege, and identity/policy driven.
- RELAY impact: team membership or local connection does not automatically authorize every project/action.

### RFC 8693 — OAuth 2.0 Token Exchange

- Year: 2020
- Type: IETF Internet Standard
- Confidence: High
- URL: https://datatracker.ietf.org/doc/html/rfc8693
- Finding: token exchange explicitly models delegation and impersonation as different semantics and can preserve both subject and actor.
- RELAY impact: preserve delegator/actor chains instead of flattening agent actions into one human identity.

### RFC 8707 — Resource Indicators for OAuth 2.0

- Year: 2020
- Type: IETF Internet Standard
- Confidence: High
- URL: https://datatracker.ietf.org/doc/html/rfc8707
- Finding: audience restriction limits tokens to intended resources; the security section specifically discusses tenant-distinguishing resource identifiers in multi-tenant systems.
- RELAY impact: project/workspace resources should be explicit credential audiences where underlying providers support this.

### RFC 9700 — OAuth 2.0 Security Best Current Practice

- Year: 2025
- Type: IETF Best Current Practice
- Confidence: High
- URL: https://datatracker.ietf.org/doc/rfc9700/
- Finding: recommends minimum privilege, audience restriction, and sender-constrained tokens to reduce misuse after token leakage.
- RELAY impact: shared broad credentials are visibly higher-risk than project/resource-scoped credentials.

### GitHub organization repository roles and deploy-key warning

- Type: first-party platform documentation
- Confidence: High for GitHub behavior
- URL: https://docs.github.com/en/organizations/managing-user-access-to-your-organizations-repositories/managing-repository-roles/repository-roles-for-an-organization
- Finding: GitHub supports graduated repository roles and warns that possession of a deploy-key private key can retain repository access even after the creating user is removed from an organization.
- RELAY impact: connection ownership/revocation is separate from user membership/offboarding.

### SyncMind: Measuring Agent Out-of-Sync Recovery in Collaborative Software Engineering

- Year: 2025
- Type: ICML peer-reviewed
- Confidence: High
- URL: https://proceedings.mlr.press/v267/guo25l.html
- Finding: SyncBench contains 24,332 out-of-sync scenarios from 21 repositories; tested agents showed substantial recovery limitations and low collaboration willingness.
- RELAY impact: revision awareness, revalidation, conflict detection, and explicit coordination are required for shared projects.

### Kung & Robinson — On optimistic methods for concurrency control

- Year: 1981
- Type: ACM Transactions on Database Systems
- Confidence: Historical/High
- DOI: 10.1145/319566.319567
- Finding: optimistic concurrency validates transaction assumptions before commit and backs out conflicting work instead of assuming no intervening change.
- RELAY impact: approved change plans need revision/precondition validation before write.

### MITRE CWE-367 — Time-of-check Time-of-use Race Condition

- Type: MITRE weakness taxonomy
- Confidence: High for the general failure pattern
- URL: https://cwe.mitre.org/data/definitions/367
- Finding: state may change between validation and later use, invalidating the original check.
- RELAY impact: approvals and permission/state checks cannot be treated as timeless.

### Hardy — The Confused Deputy

- Year: 1988
- Type: classic capability-security paper
- Confidence: Historical/High
- URL: https://www.cs.umd.edu/~jkatz/security/downloads/capabilities.html
- Finding: a program with its own authority can be tricked into exercising that authority on another party's behalf when authority and designation are confused.
- RELAY impact: keep resource authority and delegated task scope explicit rather than allowing broad ambient authority.

### Preventing Rogue Agents Improves Multi-Agent Collaboration

- Year: 2025
- Type: ACL REALM workshop paper
- Confidence: Medium-High
- URL: https://aclanthology.org/2025.realm-1.34/
- Finding: a single mistaken agent can propagate failure through a collaborative system; monitoring/intervention improved performance in the tested environments.
- RELAY impact: multi-agent work needs bounded roles, monitoring, and normal policy checks rather than automatic trust between agents.

### NIST SP 800-92 / Rev. 1 draft — Log Management

- Years: 2006 / 2023 draft revision
- Type: U.S. government security guidance
- Confidence: High
- URLs:
  - https://csrc.nist.gov/pubs/sp/800/92/final
  - https://csrc.nist.gov/pubs/sp/800/92/r1/ipd
- Finding: log generation, transport, storage, access, retention, analysis, and disposal require deliberate management.
- RELAY impact: team audit history is itself controlled security/privacy data, not an unlimited plaintext diary.


### Bounded Agents: Delegation Security for Multi-Agent AI Systems

- Year: 2026
- Type: recent preprint
- Confidence: Medium
- Source: arXiv 2608.15888
- Finding: proposes tracking delegated authority through a principal chain with accumulated session state, scope, budgets, and external enforcement; reports large reductions in attack success on the authors' evaluated suites.
- RELAY impact: strengthens the hypothesis that delegated scope/budgets should be enforced outside model reasoning, while requiring independent validation before adopting the exact mechanism.

### Authorization Architectures for Tool-Using AI Agents

- Year: 2026
- Type: recent review preprint
- Confidence: Medium
- Source: arXiv 2609.15906
- Finding: reviews agent identity, credential lifecycle, delegation, runtime policy enforcement, prompt injection, auditability, and non-repudiation across a principal hierarchy.
- RELAY impact: supports treating human, orchestrator/client, agent, sub-agent, and tool identities as distinct layers rather than one "user."
