# DSI RELAY

Created by **RAiiNMAN**. RELAY's first-party source code and documentation are licensed under [MIT](LICENSE); see [creator credit](NOTICE-RELAY.txt).

Project readers can review the [privacy statement](PRIVACY.md), [support guide](SUPPORT.md), and [security reporting policy](SECURITY.md). The earlier public preview binary remains unpublished. A separate unsigned local Windows test package is available from the development line.

DSI RELAY is a developing model-independent control and observability layer for game projects and creative toolchains.

The product exists to make AI-assisted development cheaper, more reliable, and easier to use. RELAY should do deterministic work locally, keep large results out of chat, expose a clean headless CLI, provide a plain-language dashboard, and let multiple AI clients work through the same verified project state.

## Status

Phases 0–2 are complete. Construction continues on `development/relay-v0.1`, using [the roadmap](docs/ROADMAP.md) to order Phases 3–11. The current Windows test package installs RELAY as one local program with a Start menu launcher, background engine, CLI, purple-and-gold workspace, and Diagnostics/Debug view. The workspace guides project setup and exposes truthful readiness for UEFN, Blender, and Krita. It uses the same commands as the CLI for indexing, project controls, asset checks, and declared file checks. The Core also has durable result/context storage, local discovery, bounded creator-tool adapters, and a local MCP gateway. These are real local capabilities, but live creator sessions, lower-tier hardware, clean-account install, and signed public distribution have not been accepted. Phases 3–11 remain open and this is not a production release.

The corrected end-of-build integrated run on 2026-09-27 recorded 14 PASS, 0 FAIL, 0 BLOCKED, and 29 UNTESTED across 43 workflows on the installed local build. Missing UEFN, creator-app, or specific hardware evidence was never replaced with synthetic success. Open RELAY from the Start menu after installation; opening `crates/relayd/dashboard/index.html` directly only opens the source file and cannot connect to the engine. The unpublished preview binary stays on hold; `main` remains unmerged.

## Install the local Windows test build

1. Download or copy the unsigned `relay-0.1.0-windows-x64-local-test-installer.zip` produced by `packaging/Build-LocalTestPackage.ps1` on the development branch.
2. Extract the ZIP and double-click `Install-RELAY.cmd` inside the extracted folder.
3. Open **DSI RELAY** from the Windows Start menu. Follow the three setup steps to add a project folder and build its local index. The Diagnostics/Debug section shows status and safe support information.

The extracted folder includes `Uninstall-RELAY.cmd`. Uninstall removes the program and shortcut while preserving project files and RELAY data. This package is unsigned and intended for local testing; it is not the held public binary. [Installation details](packaging/INSTALL-UPDATE-PLAN.md) explain the per-user location and update limits.

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

Start with [the roadmap](docs/ROADMAP.md) for current development and [the documentation index](docs/INDEX.md) for the full map.

The [UEFN Verse telemetry example](examples/uefn-verse/README.md) is available as an unverified project template; compilation and live capture in UEFN remain untested.

The [integrated run plan](validation/run-plan.json) and [runner](validation/Run-Integrated.ps1) produced the local report described above. They have not been used to approve a release.

- docs/PHASE3_START_HERE.md — Phase 3 background and open evidence
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

Public release is a design constraint from the beginning, not a promise of immediate release. RELAY must avoid personal paths, private credentials, user-specific defaults, and single-project assumptions. MIT and RAiiNMAN attribution apply to first-party RELAY source and documentation. Packaging, update distribution, third-party component licenses, and contribution policy still need release work.
