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
- `PERFORMANCE_AND_RESOURCE_ECONOMICS.md` — foreground priority, indexing scale, local AI contention, storage maintenance, hardware tiers, and resource budgets.
- `PHASE1_START_HERE.md` — closed Phase 1 spike/stack-selection record and handoff.
- `PHASE2_START_HERE.md` — closed Phase 2 implementation record and promotion order.
- `PHASE2_FIRST_SLICE.md` — acceptance contract for the first production-shaped Core vertical.
- `PHASE2_FIRST_SLICE_EVIDENCE.md` — accepted first-slice verification, restart, compatibility, and resource evidence.
- `PHASE2_SECOND_SLICE.md` — acceptance contract for authority, policy, transaction, usage, egress, and credential-handle foundations.
- `PHASE2_SECOND_SLICE_EVIDENCE.md` — accepted second-slice verification, migration, restart, security, and resource evidence.
- `PHASE2_THIRD_SLICE.md` — acceptance contract for generic adapter lifecycle and strong Windows worker isolation promotion.
- `PHASE2_THIRD_SLICE_EVIDENCE.md` — accepted third-slice adapter lifecycle, sandbox, concurrency, and resource evidence.
- `PHASE2_PROMOTION_LEDGER.md` — maps Phase 1 evidence to deliberate Phase 2 production promotion gates.
- `PHASE2_CLOSURE_REVIEW.md` — final Phase 2 exit-criteria review and production-foundation closure record.
- `PHASE3_START_HERE.md` — active Phase 3 project discovery/indexing implementation authority.
- `PHASE3_FIRST_SLICE.md` — acceptance contract for the first generic multi-project discovery/baseline/changed-only indexing vertical.
- `PHASE3_FIRST_SLICE_EVIDENCE.md` — accepted synthetic two-project verification, migration, restart, isolation, and resource evidence.
- `PHASE3_SECOND_SLICE_EVIDENCE.md` — accepted schema-5 change-delta, dependency-edge, migration, isolation, and restart evidence.
- `PHASE3_THIRD_SLICE_EVIDENCE.md` — accepted provisional targeted hint-update, stale-state, recovery, and scale-fixture evidence.
- `PHASE3_FOURTH_SLICE_EVIDENCE.md` — accepted explicit full-content verification for uncertain index continuity.
- `PHASE3_FIFTH_SLICE_EVIDENCE.md` — accepted Windows watcher delivery, bounded hint batching, stale-state signals, and restart evidence.
- `PHASE3_SCALE_BENCHMARK_EVIDENCE.md` — one-host 15,000-file production-fixture resource sample and repeatable tier harness.
- `PHASE3_SIXTH_SLICE_EVIDENCE.md` — durable schema-6 continuity requirement and bounded idle recovery evidence.
- `PHASE3_SEVENTH_SLICE_EVIDENCE.md` — schema-7 project configuration and declared adapter version metadata evidence.
- `PHASE3_EIGHTH_SLICE_EVIDENCE.md` — sandboxed generic dependency parser operation and bounded observation validation evidence.
- `PHASE3_CLOSURE_REVIEW.md` — active Phase 3 exit-gate review with explicit open and partial items.
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
