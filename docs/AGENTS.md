# Purpose

Own durable planning, architecture, research, roadmap, security, UX, and operating documentation for RELAY.

# Ownership

- This file governs `docs/` and all descendants unless a closer AGENTS.md exists.
- Root AGENTS.md remains authoritative for project-wide DOX rules and durable user preferences.

# Local Contracts

- Separate approved product contracts from research hypotheses and future ideas.
- Keep RELAY provider-independent, multi-project, public-ready, and cost-first in all durable documentation.
- Treat UEFN/Fortnite as the first integration target, not the permanent product boundary.
- Keep source-of-truth architecture consistent across documents: RELAY Core owns behavior; CLI, dashboard, skills, gateways, APIs, and MCP are interfaces or adapters.
- Do not embed personal paths, credentials, private machine assumptions, or project-specific behavior in public-facing design.
- When a decision changes, update the owning document instead of appending contradictory history.
- Use DECISION_LOG.md for concise durable decisions; use ROADMAP.md for delivery order; use RESEARCH_PLAN.md for unresolved research.


# Documentation Map

- `INDEX.md` — navigation for the documentation set.
- `PROJECT_PLAN.md` — consolidated product and implementation plan.
- `PRODUCT_VISION.md` — user problem, product promises, scope, and public posture.
- `SYSTEM_ARCHITECTURE.md` — component boundaries, command system, jobs, results, storage, and adapters.
- `COST_AND_CONTEXT.md` — usage-efficiency rules, Result Store, Context Compiler, memory tiers, and benchmarks.
- `CLI_AND_SKILLS.md` — headless CLI, structured machine execution, compact skills, and thin MCP posture.
- `AUTOMATION_AND_UX.md` — onboarding automation, dashboard, recovery, approvals, and support experience.
- `UEFN_V0.1.md` — first integration, runtime bridge, probes, tests, captures, and asset workflow.
- `SECURITY_AND_PERMISSIONS.md` — side-effect categories, transactions, rollback, secrets, and remote safety.
- `DATA_BOUNDARY_AND_PRIVACY.md` — data classification, credential handling, remote egress, embeddings, screenshots, and privacy boundaries.
- `RESILIENCE_AND_RECOVERY.md` — crash recovery, retries, unknown outcomes, checkpoints, backups, migrations, outages, and restore verification.
- `OBSERVABILITY_AND_TRUTH.md` — evidence classes, sampling, freshness, causality, instrumentation overhead, alert quality, and observability health.
- `SIMPLICITY_AND_OPERABILITY.md` — golden path, secure defaults, configuration/feature budgets, diagnostics, progressive depth, toil, and complexity governance.
- `LEGAL_LICENSING_AND_DISTRIBUTION.md` — platform terms, redistribution, copyleft boundaries, asset provenance, AI output rights, privacy claims, and public-release legal gates.
- `VERSIONING_AND_COMPATIBILITY.md` — contract stability, version skew, schema evolution, deprecation, migrations, support windows, and compatibility debt.
- `ROADMAP.md` — staged delivery plan and milestone exit criteria.
- `RESEARCH_PLAN.md` — academic/technical research and benchmark program.
- `PHASE0_ADVERSARIAL_REVIEW.md` — adversarial challenge to current assumptions and required architecture changes.
- `EVIDENCE_REGISTER.md` — source register tying academic, government, historical, and platform evidence to RELAY decisions.
- `DECISION_LOG.md` — approved durable architecture/product decisions.

# Work Guidance

- Prefer diagrams, contracts, invariants, and explicit boundaries over aspirational prose.
- Mark unresolved items as open questions rather than silently choosing an implementation.
- Keep examples generic unless they are explicitly labeled as fixtures or dogfood examples.

# Verification

- Re-read all changed documentation for contradictions with root AGENTS.md and sibling architecture docs.
- Verify links between docs and refresh this folder's index when documents are added, moved, renamed, or removed.

# Child DOX Index

- No child DOX files currently exist under `docs/`; this file owns the full documentation subtree.
