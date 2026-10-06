<!-- project-centered:start -->
<div align="center">

<a name="readme-top"></a>
<h1 align="center">DSI RELAY</h1>

<!-- project-header:start -->
<p align="center"><img src="readme-banner.png" alt="DSI-RELAY — original generated project artwork" width="100%"></p>
<!-- project-header:end -->

<!-- project-badges:start -->
<p align="center"><a href="https://github.com/raiinman/DSI-RELAY"><img src="https://img.shields.io/badge/DSI-RELAY-7C3AED?labelColor=333333&amp;logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAxNiAxNiI%2BPHBhdGggZD0iTTEgM2g1djVIMXpNMTAgMWg1djVoLTV6TTEwIDEwaDV2NWgtNXpNNiA1aDR2Mkg2em0xIDFoMnY2aDJ2Mkg3VjZ6IiBmaWxsPSJ3aGl0ZSIvPjwvc3ZnPg%3D%3D&amp;logoColor=white" alt="DSI: RELAY"></a> <a href="https://github.com/raiinman/DSI-RELAY"><img src="https://img.shields.io/badge/design-CLI_%2B_dashboard-0891B2?labelColor=333333" alt="design: CLI + dashboard"></a> <a href="https://github.com/raiinman/DSI-RELAY"><img src="https://img.shields.io/badge/focus-local_computation-475569?labelColor=333333" alt="focus: local computation"></a></p>
<!-- project-badges:end -->

<!-- project-live-badges:start -->
<p align="center"><a href="https://github.com/raiinman/DSI-RELAY/commits/main"><img src="https://img.shields.io/github/last-commit/raiinman/DSI-RELAY/main?labelColor=333333&amp;color=7C3AED&amp;logo=github&amp;logoColor=white&amp;label=updated" alt="last commit"></a> <a href="https://github.com/raiinman/DSI-RELAY/issues"><img src="https://img.shields.io/github/issues/raiinman/DSI-RELAY?labelColor=333333&amp;color=7C3AED&amp;logo=github&amp;logoColor=white&amp;label=issues" alt="open issues"></a> <a href="https://github.com/raiinman/DSI-RELAY/stargazers"><img src="https://badgen.net/github/stars/raiinman/DSI-RELAY?icon=github&amp;color=7C3AED&amp;labelColor=333333&amp;label=stars" alt="stars"></a></p>
<!-- project-live-badges:end -->

<!-- project-index:start -->
<a name="readme-index"></a>
<h3 align="center">✦ Explore this project</h3>

<table align="center"><tbody><tr><td align="center"><a href="#readme-overview"><strong>Overview</strong></a></td><td align="center"><a href="#readme-status"><strong>Status</strong></a></td></tr><tr><td align="center"><a href="#readme-core-product-rules"><strong>Core product rules</strong></a></td><td align="center"><a href="#readme-high-level-shape"><strong>High-level shape</strong></a></td></tr><tr><td align="center"><a href="#readme-documentation"><strong>Documentation</strong></a></td><td align="center"><a href="#readme-public-release-posture"><strong>Public-release posture</strong></a></td></tr></tbody></table>

<h4 align="center">Project shortcuts</h4>

<table align="center"><tbody><tr><td align="center"><a href="docs/PHASE1_START_HERE.md"><strong>Start here</strong></a></td><td align="center"><a href="docs/INDEX.md"><strong>Documentation hub</strong></a></td></tr><tr><td align="center"><a href="docs/SYSTEM_ARCHITECTURE.md"><strong>Architecture</strong></a></td><td align="center"><a href="docs/ROADMAP.md"><strong>Roadmap</strong></a></td></tr></tbody></table>
<!-- project-index:end -->

<a name="readme-overview"></a>
<h2 align="center">Overview</h2>

DSI RELAY is a planned model-independent development control and observability layer for game projects and creative toolchains.

The product exists to make AI-assisted development cheaper, more reliable, and easier to use. RELAY should do deterministic work locally, keep large results out of chat, expose a clean headless CLI, provide a plain-language dashboard, and let multiple AI clients work through the same verified project state.

<p align="center"><a href="#readme-index">↑ Back to index</a></p>

<a name="readme-status"></a>
## Status


Phase 0 is complete. Phase 1 technical spike and stack selection is active; implementation begins with minimal benchmark-driven prototypes.

<p align="center"><a href="#readme-index">↑ Back to index</a></p>

<a name="readme-core-product-rules"></a>
## Core product rules


<table align="center"><tbody><tr><td align="center">RELAY is not owned by any AI provider.</td></tr><tr><td align="center">RELAY Core is the source of truth for behavior.</td></tr><tr><td align="center">Headless execution and the CLI are first-class.</td></tr><tr><td align="center">The dashboard is first-class, but never the only way to perform an operation.</td></tr><tr><td align="center">Local deterministic computation is preferred over AI reasoning.</td></tr><tr><td align="center">AI-facing output is compact, structured, budget-aware, and progressively retrievable.</td></tr><tr><td align="center">MCP is a compatibility/transport adapter, not the foundation.</td></tr><tr><td align="center">Compact skills plus CLI are the initial local-AI strategy; thin/dynamic MCP remains a benchmarked alternative where it performs better.</td></tr><tr><td align="center">Projects are isolated from one another.</td></tr><tr><td align="center">UEFN/Fortnite is the first integration target, not the permanent product boundary.</td></tr><tr><td align="center">The architecture must be suitable for eventual public release.</td></tr></tbody></table>


<p align="center"><a href="#readme-index">↑ Back to index</a></p>

<a name="readme-high-level-shape"></a>
## High-level shape


<table align="center"><tbody><tr><td align="left"><pre><code>AI clients / humans
        |
        +-- CLI + skills
        +-- Dashboard
        +-- Remote gateway
        +-- Thin MCP/API adapters
        |
        v
     RELAY Core
     Command Bus
        |
        +-- project registry
        +-- jobs and transactions
        +-- result store
        +-- context compiler
        +-- tests and validation
        +-- telemetry and probes
        +-- asset registry
        +-- usage accounting
        |
        v
 integrations/adapters
        |
        +-- UEFN / Fortnite runtime
        +-- Blender
        +-- Krita
        +-- future engines and tools</code></pre></td></tr></tbody></table>


<p align="center"><a href="#readme-index">↑ Back to index</a></p>

<a name="readme-documentation"></a>
## Documentation


Start with docs/PHASE1_START_HERE.md.

<table align="center"><tbody><tr><td align="center"><a href="docs/PHASE1_START_HERE.md">docs/PHASE1_START_HERE.md</a> — active Phase 1 execution order and fresh-chat handoff</td></tr><tr><td align="center"><a href="docs/INDEX.md">docs/INDEX.md</a> — documentation map</td></tr><tr><td align="center"><a href="docs/PRODUCT_VISION.md">docs/PRODUCT_VISION.md</a> — product intent and boundaries</td></tr><tr><td align="center"><a href="docs/SYSTEM_ARCHITECTURE.md">docs/SYSTEM_ARCHITECTURE.md</a> — target technical architecture</td></tr><tr><td align="center"><a href="docs/COST_AND_CONTEXT.md">docs/COST_AND_CONTEXT.md</a> — usage-efficiency and context strategy</td></tr><tr><td align="center"><a href="docs/CLI_AND_SKILLS.md">docs/CLI_AND_SKILLS.md</a> — headless and AI invocation strategy</td></tr><tr><td align="center"><a href="docs/AUTOMATION_AND_UX.md">docs/AUTOMATION_AND_UX.md</a> — onboarding, automation, dashboard, recovery</td></tr><tr><td align="center"><a href="docs/UEFN_V0.1.md">docs/UEFN_V0.1.md</a> — first integration and v0.1 scope</td></tr><tr><td align="center"><a href="docs/SECURITY_AND_PERMISSIONS.md">docs/SECURITY_AND_PERMISSIONS.md</a> — approvals, safety, auditability</td></tr><tr><td align="center"><a href="docs/DATA_BOUNDARY_AND_PRIVACY.md">docs/DATA_BOUNDARY_AND_PRIVACY.md</a> — project data classification, credentials, remote egress, embeddings, screenshots, and privacy</td></tr><tr><td align="center"><a href="docs/RESILIENCE_AND_RECOVERY.md">docs/RESILIENCE_AND_RECOVERY.md</a> — crash recovery, unknown effects, durable jobs, backups, migrations, and outage handling</td></tr><tr><td align="center"><a href="docs/OBSERVABILITY_AND_TRUTH.md">docs/OBSERVABILITY_AND_TRUTH.md</a> — evidence quality, sampling, causality, monitoring overhead, and truth status</td></tr><tr><td align="center"><a href="docs/SIMPLICITY_AND_OPERABILITY.md">docs/SIMPLICITY_AND_OPERABILITY.md</a> — golden path, secure defaults, configuration/feature budgets, diagnostics, and product complexity</td></tr><tr><td align="center"><a href="docs/LEGAL_LICENSING_AND_DISTRIBUTION.md">docs/LEGAL_LICENSING_AND_DISTRIBUTION.md</a> — platform terms, licenses, redistribution, asset provenance, AI output rights, and public-release legal gates</td></tr><tr><td align="center"><a href="docs/VERSIONING_AND_COMPATIBILITY.md">docs/VERSIONING_AND_COMPATIBILITY.md</a> — contract stability, version skew, schema evolution, deprecation, migrations, and support lifecycle</td></tr><tr><td align="center"><a href="docs/PERFORMANCE_AND_RESOURCE_ECONOMICS.md">docs/PERFORMANCE_AND_RESOURCE_ECONOMICS.md</a> — foreground performance, indexing scale, local AI contention, storage maintenance, and hardware budgets</td></tr><tr><td align="center"><a href="docs/ROADMAP.md">docs/ROADMAP.md</a> — staged delivery plan</td></tr><tr><td align="center"><a href="docs/RESEARCH_PLAN.md">docs/RESEARCH_PLAN.md</a> — academic and technical research program</td></tr><tr><td align="center"><a href="docs/PHASE0_ADVERSARIAL_REVIEW.md">docs/PHASE0_ADVERSARIAL_REVIEW.md</a> — red-team review of current assumptions</td></tr><tr><td align="center"><a href="docs/EVIDENCE_REGISTER.md">docs/EVIDENCE_REGISTER.md</a> — academic, government, historical, and platform evidence tied to decisions</td></tr><tr><td align="center"><a href="docs/DECISION_LOG.md">docs/DECISION_LOG.md</a> — approved/revalidated durable decisions</td></tr></tbody></table>


<p align="center"><a href="#readme-index">↑ Back to index</a></p>

<a name="readme-public-release-posture"></a>
## Public-release posture


Public release is a design constraint from the beginning, not a promise of immediate release. RELAY must avoid personal paths, private credentials, user-specific defaults, and single-project assumptions. Licensing, packaging, update distribution, and contribution policy remain open decisions.

<p align="center"><a href="#readme-index">↑ Back to index</a> · <a href="#readme-top">Back to top ↑</a></p>

</div>
<!-- project-centered:end -->
