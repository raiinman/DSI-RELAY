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


## Resilience, crash recovery, retries, and migration evidence

### NIST SP 800-184 — Guide for Cybersecurity Event Recovery

- Year: 2016
- Type: U.S. government recovery guidance
- Confidence: High
- URL: https://csrc.nist.gov/pubs/sp/800/184/final
- Finding: recovery planning should include prioritized resources, playbooks, realistic test scenarios, metrics, and improvement from lessons learned.
- RELAY impact: recovery is a tested lifecycle, not merely process restart.

### NIST CSF 2.0 — Recover function

- Year: 2024
- Type: U.S. government cybersecurity framework
- Confidence: High
- URL: https://csrc.nist.gov/pubs/cswp/29/the-nist-cybersecurity-framework-csf-20/final
- Finding: RC.RP calls for scoped/prioritized recovery, verification of restoration assets before use, verification of restored assets, and confirmation of normal operating status.
- RELAY impact: restored RELAY state must be verified/reconciled before claiming healthy operation.

### NIST SP 800-34 Rev. 1 — Contingency Planning Guide

- Year: 2010
- Type: U.S. government contingency-planning guidance
- Confidence: High
- URL: https://csrc.nist.gov/pubs/sp/800/34/r1/upd1/final
- Finding: defines RTO/RPO concepts and ties recovery strategy to tolerable outage/data loss.
- RELAY impact: durability and recovery targets should differ by state class instead of using one global policy.

### NIST SP 800-160 Vol. 2 Rev. 1 — Developing Cyber-Resilient Systems

- Year: 2021
- Type: U.S. government systems-security engineering guidance
- Confidence: High
- URL: https://csrc.nist.gov/pubs/sp/800/160/v2/r1/final
- Finding: cyber resiliency includes anticipating, withstanding, recovering from, and adapting to adverse conditions while preserving mission/business functions.
- RELAY impact: degraded capability is a legitimate resilience mode; "process restarted" is not sufficient.

### CISA StopRansomware Guide

- Type: U.S. government operational guidance
- Confidence: High
- URL: https://www.cisa.gov/stopransomware/ransomware-guide
- Finding: recommends offline/encrypted backups and regular testing of backup availability/integrity under disaster-recovery conditions.
- RELAY impact: backup success and tested restore capability are separate states.

### AWS — Making retries safe with idempotent APIs

- Type: first-party distributed-systems engineering guidance
- Confidence: High for AWS experience pattern
- URL: https://aws.amazon.com/builders-library/making-retries-safe-with-idempotent-APIs/
- Finding: a timeout can leave callers unsure whether a side effect occurred; idempotent request identifiers and reconciliation reduce unsafe duplicate effects.
- RELAY impact: unknown outcome and stable idempotency keys become first-class command semantics.

### AWS Durable Execution — Idempotency and retries

- Year: current documentation
- Type: first-party workflow runtime guidance
- Confidence: High for documented semantics
- URL: https://docs.aws.amazon.com/durable-execution/patterns/best-practices/idempotency/
- Finding: replay/retry can execute side effects multiple times; at-least-once and at-most-once retry semantics do not automatically guarantee exactly-once execution across a workflow.
- RELAY impact: durable job recovery and external-effect safety are separate design problems.

### Amazon SQS — At-least-once delivery

- Type: first-party queue documentation
- Confidence: High
- URL: https://docs.aws.amazon.com/AWSSimpleQueueService/latest/SQSDeveloperGuide/standard-queues-at-least-once-delivery.html
- Finding: standard queues can deliver a message more than once; consumers should be idempotent.
- RELAY impact: duplicate queued work must be assumed unless a stronger end-to-end guarantee is proven.

### CrashMonkey / bounded black-box crash testing

- Years: 2017–2018
- Type: USENIX systems research
- Confidence: High
- URLs:
  - https://www.usenix.org/conference/hotstorage17/program/presentation/martinez
  - https://www.usenix.org/conference/osdi18/presentation/mohan
- Finding: deliberate crash testing reproduced most known crash-consistency bugs in the studied set and found new bugs in mature file systems.
- RELAY impact: recovery correctness needs kill/power-loss/fault injection, not only ordinary unit tests.

### Fractal — Fault-Tolerant Shell-Script Distribution

- Year: 2026
- Type: NSDI peer-reviewed
- Confidence: High
- URL: https://www.usenix.org/conference/nsdi26/presentation/huang
- Finding: fault recovery explicitly distinguishes recoverable computation from side-effectful regions and tracks progress/dependencies to prevent unsafe repeated effects.
- RELAY impact: command workflows need explicit durable boundaries around external side effects.

### SQLite atomic commit and corruption guidance

- Type: first-party database documentation
- Confidence: High for SQLite behavior
- URLs:
  - https://sqlite.org/atomiccommit.html
  - https://www.sqlite.org/howtocorrupt.html
- Finding: crash recovery depends on preserving transaction/journal state correctly; moving or losing a hot journal can prevent automatic recovery.
- RELAY impact: storage recovery must follow the chosen database's actual durability model and be fault tested.

### PostgreSQL pg_upgrade

- Type: first-party database documentation
- Confidence: High for PostgreSQL behavior
- URL: https://www.postgresql.org/docs/current/pgupgrade.html
- Finding: some upgrade modes can make the previous cluster unsafe to restart after the new version writes to shared/migrated data, requiring restore from backup.
- RELAY impact: application rollback and data rollback are distinct; migrations need recovery points and compatibility gates.

### Microsoft MSIX downgrade documentation

- Type: first-party Windows packaging documentation
- Confidence: High
- URL: https://learn.microsoft.com/en-us/windows/msix/desktop/managing-your-msix-deployment-downgrading
- Finding: downgrading application binaries preserves app data, and data written by the newer version may not be backward compatible.
- RELAY impact: package rollback cannot be assumed to reverse RELAY data/schema migrations.


## Observability, monitoring, and evidence-quality sources

### NIST AI 800-4 — Challenges to the Monitoring of Deployed AI Systems

- Year: 2026
- Type: U.S. government report
- Confidence: High
- URL: https://www.nist.gov/publications/challenges-monitoring-deployed-ai-systems-center-ai-standards-and-innovation
- Finding: real-world post-deployment monitoring is needed to validate expected behavior and surface unforeseen outputs/consequences; validated monitoring methods and terminology remain immature and fragmented.
- RELAY impact: monitoring output is evidence requiring coverage/quality metadata, not automatic truth.

### NIST AI 800-3 — Expanding the AI Evaluation Toolbox with Statistical Models

- Year: 2026
- Type: U.S. government measurement-science report
- Confidence: High
- URL: https://www.nist.gov/publications/expanding-ai-evaluation-toolbox-statistical-models
- Finding: common evaluation approaches can make hidden assumptions or produce invalid uncertainty estimates; fixed-benchmark and generalized performance are distinct measurement targets.
- RELAY impact: learned/probabilistic findings need uncertainty/calibration context and field performance should not be inferred from one benchmark score.

### NIST SP 800-137 / 800-137A — Information Security Continuous Monitoring

- Years: 2011 / 2020
- Type: U.S. government continuous-monitoring guidance
- Confidence: High
- URLs:
  - https://csrc.nist.gov/pubs/sp/800/137/final
  - https://csrc.nist.gov/pubs/sp/800/137/a/final
- Finding: monitoring is an ongoing assurance program whose strategy, operations, data, completeness, and effectiveness themselves require assessment.
- RELAY impact: the observability pipeline needs health/coverage monitoring rather than being assumed functional.

### NIST SP 800-92 — Guide to Computer Security Log Management

- Year: 2006
- Type: U.S. government log-management guidance
- Confidence: High
- URL: https://csrc.nist.gov/pubs/sp/800/92/final
- Finding: effective logs require deliberate collection, infrastructure, management, analysis, storage, and lifecycle processes.
- RELAY impact: logs are managed evidence with provenance/retention/quality concerns, not an infallible truth stream.

### Gray Failure: The Achilles' Heel of Cloud-Scale Systems

- Year: 2017
- Type: HotOS / Microsoft production-systems research
- Confidence: High
- URL: https://www.microsoft.com/en-us/research/publication/gray-failure-achilles-heel-cloud-scale-systems/
- Finding: production systems can suffer "differential observability," where applications experience failure while failure detectors do not see the problem.
- RELAY impact: quiet health checks are not proof of healthy project/tool behavior; reconcile multiple perspectives.

### OpenTelemetry / W3C Trace Context sampling and security

- Type: first-party/open standard documentation
- Confidence: High for protocol semantics
- URLs:
  - https://opentelemetry.io/docs/specs/otel/trace/tracestate-probability-sampling/
  - https://opentelemetry.io/docs/concepts/sampling/
  - https://www.w3.org/TR/trace-context/
- Finding: sampling intentionally drops trace data; inconsistent decisions can create fragmented/unusable traces. W3C warns tracing metadata can be abused to create monitoring overhead, trace collisions, or monitoring denial.
- RELAY impact: sampling/completeness must be visible, exact counts must respect sampling, and observability metadata remains untrusted input.

### An empirical study on the performance overhead of code instrumentation in containerised microservices

- Year: 2025
- Type: Journal of Systems and Software peer-reviewed study
- Confidence: High
- DOI: 10.1016/j.jss.2025.112573
- Finding: more than 5,000 experiments found measurable performance overhead from automatic observability instrumentation, including substantial effects in heavier cases.
- RELAY impact: instrumentation/probes need overhead budgets and performance diagnostics must account for observer effects.

### Investigating Performance Overhead of Distributed Tracing in Microservices and Serverless Systems

- Year: 2025
- Type: ICPE Companion paper
- Confidence: Medium-High
- URL: https://atlarge-research.com/pdfs/2025-tracing-overhead-anou.pdf
- Finding: distributed tracing materially changed throughput and latency across evaluated configurations.
- RELAY impact: reinforces targeted instrumentation and benchmarked low/high detail modes.

### Quantifying and mitigating alarm fatigue caused by fault detection systems

- Year: 2026
- Type: Reliability Engineering & System Safety peer-reviewed
- Confidence: High
- DOI: 10.1016/j.ress.2025.111890
- Finding: repeated false alarms create a cry-wolf effect; common detector metrics do not fully capture operational usability.
- RELAY impact: alert volume, duplication, false discovery/noise, and operator response matter alongside model metrics.

### Research note: The impact of absolute false positive rates on operational reliability

- Year: 2026
- Type: Reliability Engineering & System Safety peer-reviewed short communication
- Confidence: High
- DOI: 10.1016/j.ress.2026.112760
- Finding: conventional accuracy/precision/recall/F1 can mask operationally unacceptable absolute false-positive volumes.
- RELAY impact: detector dashboards need absolute alert-rate metrics, not only benchmark percentages.

### Time, Causality, and Observability Failures in Distributed AI Inference Systems

- Year: 2026
- Type: recent preprint
- Confidence: Medium
- Source: arXiv 2604.21361
- Finding: controlled experiments found small clock skew could make traces causally inconsistent while the underlying distributed AI pipeline remained functionally correct.
- RELAY impact: wall-clock ordering is insufficient for causal claims; prefer explicit relationships/sequence data and record clock quality.

### Accurate Distributed Tracing for Large-Scale AI Infrastructure

- Year: 2026
- Type: very recent preprint
- Confidence: Medium-Low pending independent validation
- Source: arXiv 2609.23301
- Finding: argues that insufficient time synchronization can create causal inversions and unreliable fault attribution in large AI systems.
- RELAY impact: strengthens clock-quality/causal-order research, but precise reported thresholds should not become RELAY defaults without replication.

### NASA sensor-validation work

- Years: 1990s
- Type: U.S. government/NASA technology reports
- Confidence: Historical
- URLs:
  - https://spinoff.nasa.gov/node/10047
  - https://ntrs.nasa.gov/citations/20050180660
- Finding: mission-critical automation required explicit validation of sensor inputs because bad sensor data caused operational disruption.
- RELAY impact: the older lesson still applies: validate evidence sources rather than trusting instrumentation merely because it is automated.

### Goodhart-style metric distortion

- Type: established measurement/organizational phenomenon with empirical studies
- Confidence: Historical/High for general risk
- Example: https://pmc.ncbi.nlm.nih.gov/articles/PMC6541803/
- Finding: when measures become targets, behavior can optimize the metric rather than the intended outcome.
- RELAY impact: do not optimize token savings, cache hit rate, alert count, or AI-call reduction without correctness/safety guardrails.


## Simplicity, cognitive load, configuration, and operability evidence

### NIST — What is Human-Centered Cybersecurity?

- Year: 2026
- Type: NIST/SOUPS conference poster
- Confidence: High for direction, preliminary for findings
- URL: https://www.nist.gov/publications/what-human-centered-cybersecurity
- Finding: NIST is explicitly developing a shared human-centered cybersecurity approach rather than treating the human as an afterthought.
- RELAY impact: human cognitive/operational burden belongs in architecture and evaluation, not only UI polish.

### NIST — Advancing Human-Centered Cybersecurity

- Year: 2026
- Type: IEEE Security & Privacy / NIST authorship
- Confidence: High
- URL: https://www.nist.gov/publications/advancing-human-centered-cybersecurity-challenges-and-pathways-forward
- Finding: summarizes contemporary research/practice challenges in integrating human factors into cybersecurity outcomes.
- RELAY impact: security and usability constraints must be co-designed.

### NIST SP 1332 — ConnectCon 2024 Human-Centered Cybersecurity Workshop Summary

- Year: 2025
- Type: U.S. government workshop report
- Confidence: High for identified practitioner/research themes
- URL: https://www.nist.gov/publications/workshop-summary-report-connectcon-2024-minding-gaps-human-centered-cybersecurity
- Finding: identifies human-centered cybersecurity challenges and pathways based on practitioner/researcher consensus.
- RELAY impact: reinforces representative-user evaluation of security controls and workflows.

### NIST — Security Fatigue

- Year: 2016
- Type: peer-reviewed NIST study
- Confidence: High
- URL: https://csrc.nist.gov/pubs/journal/2016/09/security-fatigue/final
- Finding: more than half of 40 interviewed users alluded to security fatigue; themes included resignation, loss of control, decision avoidance, and choosing easier options.
- RELAY impact: repeated security decisions and warnings can undermine safe behavior.

### CISA/NSA/FBI — Secure by Design / Secure by Default guidance

- Years: 2023–2025
- Type: joint government security guidance
- Confidence: High for product-design principle
- URLs:
  - https://www.cisa.gov/sites/default/files/2023-06/principles_approaches_for_security-by-design-default_508c.pdf
  - https://www.cisa.gov/news-events/cybersecurity-advisories/aa23-278a
- Finding: secure configuration should be the default baseline and configuration complexity should not be pushed onto customers.
- RELAY impact: safe defaults and low-configuration golden paths are security requirements, not just UX preferences.

### CISA — Secure-by-Design alert on default passwords

- Year: 2023/updated guidance
- Type: U.S. government product-design guidance
- Confidence: High
- URL: https://www.cisa.gov/sites/default/files/2023-12/SbD-Alert-How-Software-Manufacturers-Can-Protect-Customers-by-Eliminating-Default-Passwords-508c_0.pdf
- Finding: manufacturers should make the easiest route the secure one and field-test how customers actually deploy products.
- RELAY impact: auto-configuration needs inspectability, field testing, and safe defaults.

### Measuring the cognitive load of software developers: an extended systematic mapping study

- Year: 2021
- Type: Information and Software Technology peer-reviewed review
- Confidence: High
- DOI: 10.1016/j.infsof.2021.106563
- Finding: reviewed 63 primary studies measuring developer cognitive load and found programming tasks are a major focus, while measurement remains methodologically challenging.
- RELAY impact: cognitive load is measurable enough to evaluate, but no single metric should be treated as definitive.

### When Help Hurts: Verification Load and Fatigue with AI Coding Assistants

- Year: 2026
- Type: CHI peer-reviewed
- Confidence: High
- DOI: 10.1145/3772318.3791176
- Finding: with one fixed model backend, interaction mode materially changed correctness, time, workload, and verification burden for 60 developers.
- RELAY impact: interface/workflow design can change AI-assistance cost even when model quality is unchanged.

### Platform engineering and internal developer portals: a multivocal literature review

- Year: 2026
- Type: peer-reviewed multivocal literature review
- Confidence: Medium-High; field remains immature
- URL: https://www.frontiersin.org/journals/computer-science/articles/10.3389/fcomp.2026.1814498/full
- Finding: platforms aim to reduce cognitive load but can become sources of complexity themselves; the review notes evidence quality drops for many specific platform practices.
- RELAY impact: RELAY must be managed as a product that can itself create cognitive load.

### Can There Ever Be Too Many Options? A Meta-Analytic Review of Choice Overload

- Year: 2010
- Type: Journal of Consumer Research meta-analysis
- Confidence: High
- DOI: 10.1086/651235
- Finding: across 50 published/unpublished experiments, mean choice-overload effect was near zero with substantial variability.
- RELAY impact: "fewer options" is not a universal UX law.

### Choice overload: A conceptual review and meta-analysis

- Year: 2015
- Type: Journal of Consumer Psychology meta-analysis
- Confidence: High
- DOI: 10.1016/j.jcps.2014.08.002
- Finding: choice set complexity, task difficulty, preference uncertainty, and decision goal moderate overload.
- RELAY impact: contextual defaults/recommendations matter more than a crude global option limit.

### Early Detection of Configuration Errors to Reduce Failure Damage

- Year: 2016
- Type: OSDI peer-reviewed, Best Paper
- Confidence: High
- URL: https://www.usenix.org/conference/osdi16/technical-sessions/presentation/xu
- Finding: critical latent configuration errors were common in mature systems; generated initialization checks detected more than 75% of studied real-world latent configuration errors.
- RELAY impact: validate important configuration assumptions early rather than at rare failure time.

### Testing of highly configurable cyber-physical systems

- Year: 2023
- Type: Journal of Systems and Software peer-reviewed multiple-case study
- Confidence: High
- DOI: 10.1016/j.jss.2023.111624
- Finding: configuration variability makes industrial testing difficult; option dependencies are often only partially modeled and practitioners want broader automated coverage.
- RELAY impact: every configurable RELAY feature increases validation/support state.

### Test them all, is it worth it? JHipster configuration sampling

- Year: 2019
- Type: Empirical Software Engineering peer-reviewed
- Confidence: High
- DOI: 10.1007/s10664-018-9635-4
- Finding: 35.70% of evaluated JHipster configurations failed; systematic testing strategies improved fault detection but could exceed testing budgets.
- RELAY impact: supported configuration profiles and constraints may be safer than claiming full combinatorial support.

### Hidden Technical Debt in Machine Learning Systems

- Year: 2015
- Type: NeurIPS peer-reviewed
- Confidence: High
- URL: https://papers.nips.cc/paper/2015/hash/86df7dcfd896fcaf2674f757a2463eba-Abstract.html
- Finding: configuration issues, boundary erosion, entanglement, undeclared consumers, and system interactions create substantial long-term maintenance costs.
- RELAY impact: feature/integration additions must account for permanent architecture and support debt.

### Software Development Practices, Software Complexity, and Software Maintenance Performance

- Year: 1998
- Type: Management Science field study
- Confidence: Historical/High
- DOI: 10.1287/mnsc.44.4.433
- Finding: software complexity links development/design decisions to downstream maintenance performance.
- RELAY impact: implementation complexity and long-term support cost belong in feature admission decisions.

### No Silver Bullet: Essence and Accidents of Software Engineering

- Year: 1987
- Type: IEEE Computer classic software-engineering paper
- Confidence: Historical/High for conceptual framing
- DOI: 10.1109/MC.1987.1663532
- Finding: essential software complexity cannot be wished away by representation alone.
- RELAY impact: do not build premature universal abstractions that merely relocate engine/tool complexity.

### Google SRE Workbook — Eliminating Toil

- Year: 2018
- Type: established production engineering guidance
- Confidence: High for operational practice
- URL: https://research.google/pubs/the-site-reliability-engineering-workbook-chapter-eliminating-toil/
- Finding: repetitive operational work can consume teams unless measured and bounded.
- RELAY impact: maintenance/reconnection/configuration/support rituals need a toil budget and root-cause elimination.

### An empirical study of developers' challenges in Workflows as Code: Apache Airflow

- Year: 2025
- Type: Journal of Systems and Software peer-reviewed
- Confidence: High
- DOI: 10.1016/j.jss.2024.112248
- Finding: defining/executing workflows were major challenges, often caused by configuration errors, and developers relied on diverse documentation and expertise.
- RELAY impact: self-diagnostics and generated/version-consistent reference surfaces reduce configuration/documentation fragmentation.


## Legal, licensing, ownership, and public-distribution evidence

### Epic Games Terms of Service

- Current effective date: September 10, 2026
- Type: first-party platform contract
- Confidence: High for current Epic contractual text
- URL: https://legal.epicgames.com/epicgames/tos
- Finding: UEFN is a Licensed Product; Epic's general terms prohibit bot software/services used to automate Licensed Products and prohibit unauthorized reverse engineering/modification. The same terms also govern account/security and ecosystem use.
- RELAY impact: public UEFN automation must stay within clearly documented developer tooling and should not generalize to gameplay/client automation.

### Epic — Unreal MCP / UEFN MCP documentation

- Year: 2026
- Type: first-party developer documentation
- Confidence: High for current supported feature intent
- URLs:
  - https://dev.epicgames.com/documentation/fortnite/uefn-mcp
  - https://dev.epicgames.com/documentation/fortnite/42-00-fortnite-ecosystem-updates-and-release-notes
  - https://dev.epicgames.com/documentation/unreal-engine/unreal-mcp-in-unreal-editor
- Finding: Epic explicitly ships Unreal MCP in UEFN so MCP-compatible AI agents can drive the editor, write/compile Verse, manipulate scene/device state, and run play sessions; Unreal MCP remains Experimental in UE 5.8.
- RELAY impact: documented MCP is the preferred UEFN automation surface; the tension with broad general bot-language in the ToS should be treated narrowly and reviewed rather than ignored.

### UEFN Supplemental Terms

- Type: first-party platform contract
- Confidence: High for current Epic terms
- URL: https://legal.epicgames.com/epicgames/uefn
- Findings:
  - Developer-Made Content remains the developer's subject to Epic/third-party rights and the license granted to Epic.
  - developers warrant they have the rights required for submitted content.
  - Developer-Made Content/code may not establish connections to non-Epic servers after upload/download by end users.
  - terms incorporate Epic's other policies/rules and those can evolve.
- RELAY impact: no published in-island RELAY phone-home client; asset provenance matters; terms need version tracking.

### Fortnite Developer Rules and change log

- Current rules last updated: April 29, 2026
- Type: first-party platform policy
- Confidence: High
- URLs:
  - https://legal.epicgames.com/fortnite/developer-rules
  - https://legal.epicgames.com/fortnite/developer-rules-change-log
- Findings: creators are responsible for content compliance and IP rights; Epic-owned IP not made available to creators cannot simply be recreated/used; rules are actively revised.
- RELAY impact: project publishing checks need source/version attribution and cannot assume old creator rules remain current.

### Epic Fan Content Policy

- Type: first-party IP/trademark policy
- Confidence: High for current policy text
- URL: https://legal.epicgames.com/epicgames/fan-art-policy
- Finding: covered Epic-related websites/apps are described as personal, non-commercial, and freely accessible; Epic marks may not be used to identify/promote another product or imply official endorsement.
- RELAY impact: commercial/public RELAY branding must not rely on this policy as its trademark/license basis.

### Unreal Engine EULA — license compatibility and Engine Tools

- Type: first-party proprietary software license
- Confidence: High for current Unreal Engine license text
- URL: https://www.unrealengine.com/eula/unreal
- Finding: Unreal Licensed Technology cannot be combined/distributed with GPL/LGPL/CC-BY-SA in ways that would impose incompatible terms; Engine Tools have specific distribution restrictions.
- RELAY impact: an Unreal/UEFN in-process companion needs a separate license/distribution analysis; GPL host-plugin obligations from Blender/Krita cannot simply be copied into Unreal components.

### Blender license

- Type: first-party open-source project licensing statement
- Confidence: High
- URL: https://www.blender.org/about/license/
- Findings:
  - Blender source is generally GPL-2.0-or-later and binary distributions are compatible under GPL-3.0-or-later.
  - published Python add-ons using Blender's Python API must use a GPL-compatible license.
  - artwork/data created with Blender remains the creator's property.
- RELAY impact: distributed Blender companions must satisfy GPL-compatible obligations while RELAY Core licensing remains a separate architectural decision.

### Krita license

- Type: first-party open-source project licensing statement
- Confidence: High
- URL: https://krita.org/en/about/license/
- Findings:
  - Krita as a whole is GPLv3.
  - distributed plugins using Krita's extension API must be GPL.
  - artwork created in Krita remains the creator's property.
- RELAY impact: a public Krita bridge/plugin is a GPL component; source/notices/distribution obligations need explicit handling.

### U.S. Copyright Office — Copyright and Artificial Intelligence, Part 2

- Year: 2025
- Type: U.S. government copyright report
- Confidence: High for U.S. Copyright Office position
- URLs:
  - https://www.copyright.gov/newsnet/2025/1060.html
  - https://www.copyright.gov/ai/
- Finding: generative-AI outputs are copyrightable only where sufficient human-authored expressive elements exist; prompts alone generally do not provide sufficient authorship, while human-authored input, selection/arrangement, or creative modification can.
- RELAY impact: do not promise copyright ownership/copyrightability for generated assets; track provenance and separate provider contractual rights from copyright law.

### FTC — privacy promises for software/apps

- Type: U.S. government consumer-protection guidance/enforcement history
- Confidence: High for U.S. FTC expectations
- URLs:
  - https://www.ftc.gov/business-guidance/resources/marketing-your-mobile-app-get-it-right-start
  - https://www.ftc.gov/business-guidance/privacy-security/consumer-privacy
  - https://www.ftc.gov/policy/advocacy-research/tech-at-ftc/2024/02/keeping-your-privacy-enhancing-technology-pet-promises
- Finding: developers must honor privacy/security representations and should make user choices clear; privacy-enhancing claims must accurately describe what the implementation actually provides.
- RELAY impact: "local-only", privacy, security, telemetry, deletion, and diagnostic claims must match tested product behavior.

### NIST — SBOM and open-source software controls

- Type: U.S. government software-supply-chain guidance
- Confidence: High
- URLs:
  - https://www.nist.gov/itl/executive-order-14028-improving-nations-cybersecurity/software-supply-chain-security-guidance-20
  - https://www.nist.gov/itl/executive-order-14028-improving-nations-cybersecurity/software-supply-chain-security-guidance-22
- Finding: software producers/acquirers benefit from machine-readable component inventories, provenance, open-source controls, and continuously maintained supply-chain information.
- RELAY impact: public artifacts need reproducible dependency/component/license inventories, while legal license obligations remain a distinct layer from vulnerability data.


## Versioning, compatibility, schema evolution, and deprecation evidence

### NIST SP 800-228-upd1 — Guidelines for API Protection for Cloud-Native Systems

- Year: 2025 update
- Type: U.S. government API security guidance
- Confidence: High
- URL: https://tsapps.nist.gov/publication/get_pdf.cfm?pub_id=961660
- Finding: NIST recommends API inventories, well-defined specifications/IDLs, schema validation, and explicit strategies for API versioning, deprecation, and sunsetting with secure migration paths.
- RELAY impact: versioning/deprecation is a lifecycle/security concern, not just a release-number convention.

### CISA Cloud Security Technical Reference Architecture

- Type: U.S. government cloud/security architecture guidance
- Confidence: High
- URL: https://www.cisa.gov/sites/default/files/publications/Cloud%20Security%20Technical%20Reference%20Architecture.pdf
- Finding: recommends API versioning to manage changes over time and providing tenants sufficient time to transition between versions.
- RELAY impact: public remote/API compatibility needs explicit transition windows rather than surprise replacement.

### Microsoft REST/API Guidelines

- Type: mature first-party API design guidance
- Confidence: High for Microsoft practice
- URLs:
  - https://github.com/microsoft/api-guidelines
  - https://github.com/microsoft/api-guidelines/blob/vNext/azure/VersioningGuidelines.md
  - https://github.com/microsoft/api-guidelines/blob/vNext/graph/articles/deprecation.md
- Findings:
  - explicit versioning is required in the historical general guidelines
  - breaking changes include behavior/error/permission/performance changes, not only shape changes
  - Azure guidance strongly avoids changing existing API-version behavior
  - Graph deprecation guidance records deprecation date, description, and removal date
- RELAY impact: breaking-change review must cover runtime semantics and deprecation metadata/migration, not only schemas.

### RFC 9745 — Deprecation HTTP Response Header Field and RFC 8594 — Sunset

- Years: 2025 / 2019
- Type: IETF standards
- Confidence: High
- URLs:
  - https://www.rfc-editor.org/rfc/rfc9745.html
  - https://www.rfc-editor.org/rfc/rfc8594.html
- Finding: standardizes machine/human-visible signaling for deprecated HTTP resources and optional future sunset dates.
- RELAY impact: remote/API surfaces can expose structured deprecation metadata instead of relying only on release notes.

### Kubernetes API Deprecation Policy

- Type: major open-source platform compatibility policy
- Confidence: High for Kubernetes practice
- URL: https://kubernetes.io/docs/reference/deprecation-policy/
- Findings:
  - API removal requires version progression
  - behavior/significant shape is protected within an API version
  - API objects should round-trip between supported versions without information loss where specified
  - alpha/beta/stable tracks carry different promises
- RELAY impact: supports stability classes, versioned removal, and round-trip/migration testing.

### Kubernetes Version Skew Policy

- Type: major open-source platform operations policy
- Confidence: High for Kubernetes practice
- URL: https://kubernetes.io/releases/version-skew-policy/
- Finding: Kubernetes explicitly defines which component versions may coexist and the supported upgrade order.
- RELAY impact: local host, CLI, gateway, adapters, dashboard, and companions need explicit mixed-version support rather than lockstep assumptions.

### Protocol Buffers — Best Practices and schema evolution

- Type: first-party serialization documentation
- Confidence: High
- URLs:
  - https://protobuf.dev/best-practices/dos-donts/
  - https://protobuf.dev/programming-guides/proto3/
  - https://protobuf.dev/programming-guides/json/
- Findings:
  - clients/servers are never guaranteed to update simultaneously
  - retired field numbers should be reserved and not reused
  - changing certain field types/oneof structures is unsafe
  - ProtoJSON has materially weaker/different schema-evolution guarantees than binary wire format because unknown fields/names are handled differently
- RELAY impact: schema-evolution rules depend on the actual encoding; retired identifiers and unknown-field semantics matter.

### An extended study of syntactic breaking changes in the wild

- Year: 2024
- Type: Empirical Software Engineering peer-reviewed
- Confidence: High
- URL: https://link.springer.com/article/10.1007/s10664-024-10563-4
- Finding: across the studied Maven dependencies, 11.58% of automated updates produced client-affecting breaking changes and almost half of those occurred during non-major updates; transitive dependencies were a significant source.
- RELAY impact: Semantic Versioning cannot substitute for actual compatibility testing, including transitive dependency effects.

### A Large-Scale Empirical Study on Semantic Versioning in Golang Ecosystem

- Year: 2023
- Type: research preprint / large empirical study
- Confidence: Medium-High
- Source: arXiv 2309.02894
- Finding: 86.3% of studied upgrades complied with SemVer, but 28.6% of no-major upgrades introduced breaking changes and about one-third of downstream clients could be affected by breaking changes.
- RELAY impact: version-number policy is useful but not reliable enough for automated trust.

### How Java APIs break — An empirical study

- Year: 2015
- Type: Information and Software Technology peer-reviewed
- Confidence: High
- DOI: 10.1016/j.infsof.2015.02.014
- Finding: incompatible API changes were common and the study notes manually assigned/versioned compatibility schemes are error-prone because subtle changes and client/provider interpretations differ.
- RELAY impact: automated compatibility checks and explicit contract definitions are necessary.

### Semantic versioning and impact of breaking changes in the Maven repository

- Year: 2017
- Type: Journal of Systems and Software peer-reviewed
- Confidence: High
- DOI: 10.1016/j.jss.2016.04.008
- Finding: around one third of studied releases introduced at least one breaking change and deprecation tags were applied inconsistently.
- RELAY impact: deprecation/version labels alone do not guarantee safe client evolution.

### On the reaction to deprecation of clients of popular Java APIs and the JDK

- Year: 2018
- Type: Empirical Software Engineering peer-reviewed
- Confidence: High
- URL: https://link.springer.com/article/10.1007/s10664-017-9554-9
- Finding: studies client reaction to deprecated APIs and reviews prior evidence that breaking changes/deprecations appear across minor and major releases and clients can lag API evolution.
- RELAY impact: deprecated contracts need usage awareness, migration tooling, and sufficient support windows.

### Reducing the Impact of Breaking Changes to Web Service Clients During Web API Evolution

- Year: 2023
- Type: IEEE/ACM MOBILESoft peer-reviewed
- Confidence: High
- DOI: 10.1109/MOBILSoft59058.2023.00008
- Finding: migration tooling identified 1,132 breaking changes across 13 web-service version increments; some changes could be automated while hundreds required developer-authored migration guidance.
- RELAY impact: important breaking releases need machine-assisted migration plus explicit manual steps for irreducible changes.

### Feature Toggle Dynamics in Large-Scale Systems: Prevalence, Growth, Lifespan, and Benchmarking

- Year: 2026
- Type: recent preprint
- Confidence: Medium
- Source: arXiv 2604.15872
- Finding: analysis of more than 4,000 toggle events in Kubernetes/GitLab found removals lagged additions and some toggles became effectively permanent; the studied Kubernetes toggles had a long median lifespan.
- RELAY impact: compatibility/rollout flags need owners, review/removal criteria, and must not become an undocumented permanent API layer.

### NIST software/configuration management guidance

- Type: U.S. government configuration-management guidance
- Confidence: High for general change-control principles
- URLs:
  - https://nvlpubs.nist.gov/nistpubs/Legacy/SP/nistspecialpublication500-161.pdf
  - https://nvlpubs.nist.gov/nistpubs/SpecialPublications/800-171r3/NIST.SP.800-171r3.html
- Finding: NIST emphasizes controlled baselines, interface control, review/approval/testing/documentation of changes, and retention/management of configuration baselines.
- RELAY impact: public contract changes and migrations require controlled, tested baselines rather than ad hoc evolution.
