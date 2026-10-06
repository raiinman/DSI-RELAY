# DSI RELAY

<!-- project-header:start -->
![DSI-RELAY — original generated project artwork](readme-banner.png)
<!-- project-header:end -->

<!-- project-badges:start -->
[![DSI: RELAY](https://img.shields.io/badge/DSI-RELAY-7C3AED?labelColor=333333&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAxNiAxNiI%2BPHBhdGggZD0iTTEgM2g1djVIMXpNMTAgMWg1djVoLTV6TTEwIDEwaDV2NWgtNXpNNiA1aDR2Mkg2em0xIDFoMnY2aDJ2Mkg3VjZ6IiBmaWxsPSJ3aGl0ZSIvPjwvc3ZnPg%3D%3D&logoColor=white)](https://github.com/raiinman/DSI-RELAY)
[![design: CLI + dashboard](https://img.shields.io/badge/design-CLI_%2B_dashboard-0891B2?labelColor=333333)](https://github.com/raiinman/DSI-RELAY)
[![focus: local computation](https://img.shields.io/badge/focus-local_computation-475569?labelColor=333333)](https://github.com/raiinman/DSI-RELAY)
<!-- project-badges:end -->

<!-- project-live-badges:start -->
[![last commit](https://img.shields.io/github/last-commit/raiinman/DSI-RELAY/main?labelColor=333333&color=7C3AED&logo=github&logoColor=white&label=updated)](https://github.com/raiinman/DSI-RELAY/commits/main) [![open issues](https://img.shields.io/github/issues/raiinman/DSI-RELAY?labelColor=333333&color=7C3AED&logo=github&logoColor=white&label=issues)](https://github.com/raiinman/DSI-RELAY/issues) [![stars](https://badgen.net/github/stars/raiinman/DSI-RELAY?icon=github&color=7C3AED&labelColor=333333&label=stars)](https://github.com/raiinman/DSI-RELAY/stargazers)
<!-- project-live-badges:end -->

DSI RELAY is a planned model-independent development control and observability layer for game projects and creative toolchains.

The product exists to make AI-assisted development cheaper, more reliable, and easier to use. RELAY should do deterministic work locally, keep large results out of chat, expose a clean headless CLI, provide a plain-language dashboard, and let multiple AI clients work through the same verified project state.

## Status

Phase 0 is complete. Phase 1 technical spike and stack selection is active; implementation begins with minimal benchmark-driven prototypes.

## Core product rules

- RELAY is not owned by any AI provider.
- RELAY Core is the source of truth for behavior.
- Headless execution and the CLI are first-class.
- The dashboard is first-class, but never the only way to perform an operation.
- Local deterministic computation is preferred over AI reasoning.
- AI-facing output is compact, structured, budget-aware, and progressively retrievable.
- MCP is a compatibility/transport adapter, not the foundation.
- Compact skills plus CLI are the initial local-AI strategy; thin/dynamic MCP remains a benchmarked alternative where it performs better.
- Projects are isolated from one another.
- UEFN/Fortnite is the first integration target, not the permanent product boundary.
- The architecture must be suitable for eventual public release.

## High-level shape

~~~
AI clients / humans
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
        +-- future engines and tools
~~~

## Documentation

Start with docs/PHASE1_START_HERE.md.

- docs/PHASE1_START_HERE.md — active Phase 1 execution order and fresh-chat handoff
- docs/INDEX.md — documentation map
- docs/PRODUCT_VISION.md — product intent and boundaries
- docs/SYSTEM_ARCHITECTURE.md — target technical architecture
- docs/COST_AND_CONTEXT.md — usage-efficiency and context strategy
- docs/CLI_AND_SKILLS.md — headless and AI invocation strategy
- docs/AUTOMATION_AND_UX.md — onboarding, automation, dashboard, recovery
- docs/UEFN_V0.1.md — first integration and v0.1 scope
- docs/SECURITY_AND_PERMISSIONS.md — approvals, safety, auditability
- docs/DATA_BOUNDARY_AND_PRIVACY.md — project data classification, credentials, remote egress, embeddings, screenshots, and privacy
- docs/RESILIENCE_AND_RECOVERY.md — crash recovery, unknown effects, durable jobs, backups, migrations, and outage handling
- docs/OBSERVABILITY_AND_TRUTH.md — evidence quality, sampling, causality, monitoring overhead, and truth status
- docs/SIMPLICITY_AND_OPERABILITY.md — golden path, secure defaults, configuration/feature budgets, diagnostics, and product complexity
- docs/LEGAL_LICENSING_AND_DISTRIBUTION.md — platform terms, licenses, redistribution, asset provenance, AI output rights, and public-release legal gates
- docs/VERSIONING_AND_COMPATIBILITY.md — contract stability, version skew, schema evolution, deprecation, migrations, and support lifecycle
- docs/PERFORMANCE_AND_RESOURCE_ECONOMICS.md — foreground performance, indexing scale, local AI contention, storage maintenance, and hardware budgets
- docs/ROADMAP.md — staged delivery plan
- docs/RESEARCH_PLAN.md — academic and technical research program
- docs/PHASE0_ADVERSARIAL_REVIEW.md — red-team review of current assumptions
- docs/EVIDENCE_REGISTER.md — academic, government, historical, and platform evidence tied to decisions
- docs/DECISION_LOG.md — approved/revalidated durable decisions

## Public-release posture

Public release is a design constraint from the beginning, not a promise of immediate release. RELAY must avoid personal paths, private credentials, user-specific defaults, and single-project assumptions. Licensing, packaging, update distribution, and contribution policy remain open decisions.
