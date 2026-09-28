# DOX framework

- DOX is the highly performant AGENTS.md hierarchy installed here.
- Agents must follow DOX instructions across any edits.

## Core Contract

- AGENTS.md files are binding work contracts for their subtrees.
- Work products, source materials, instructions, records, assets, and durable docs must stay understandable from the nearest applicable AGENTS.md plus every parent AGENTS.md above it.
- Keep `Cargo.lock` checked out with LF line endings through `.gitattributes`; the release dependency and provenance checks pin its raw SHA-256 across Windows hosts.

## Read Before Editing

1. Read the root AGENTS.md.
2. Identify every file or folder you expect to touch.
3. Walk from the repository root to each target path.
4. Read every AGENTS.md found along each route.
5. If a parent AGENTS.md lists a child AGENTS.md whose scope contains the path, read that child and continue from there.
6. Use the nearest AGENTS.md as the local contract and parent docs for repo-wide rules.
7. If docs conflict, the closer doc controls local work details, but no child doc may weaken DOX.

Do not rely on memory. Re-read the applicable DOX chain in the current session before editing.

## Project Indexing Requirements

- Before substantive work in an unindexed or materially changed repository, scan the project recursively rather than relying on a shallow directory listing.
- Build the DOX tree from durable project boundaries, not arbitrary folder depth.
- Create a child AGENTS.md when a folder has its own durable purpose, rules, responsibilities, workflow, materials, or quality standards.
- Continue recursively inside each indexed child and create deeper child AGENTS.md files where separate durable boundaries exist.
- Every parent AGENTS.md must maintain an accurate Child DOX Index of its direct child AGENTS.md files and explain what each child owns.
- The root Child DOX Index is the authoritative top-level project index.
- Index source code, tooling, integrations, docs, assets, tests, generated artifacts, records, and operational folders when they become durable boundaries.
- Do not create child AGENTS.md files merely to mirror every directory. Prefer the smallest hierarchy that makes ownership and local contracts obvious.
- When paths are added, removed, moved, renamed, or repurposed, refresh every affected parent index in the same task.
- An index entry must use the exact repository-relative child path and a concise statement of scope.
- If a repository is currently empty or has no durable child boundaries, state that explicitly in the Child DOX Index rather than leaving a placeholder.

## Update After Editing

Every meaningful change requires a DOX pass before the task is done.

Update the closest owning AGENTS.md when a change affects:

- purpose, scope, ownership, or responsibilities
- durable structure, contracts, workflows, or operating rules
- required inputs, outputs, permissions, constraints, side effects, or artifacts
- user preferences about behavior, communication, process, organization, or quality
- AGENTS.md creation, deletion, move, rename, or index contents

Update parent docs when parent-level structure, ownership, workflow, or child index changes. Update child docs when parent changes alter local rules. Remove stale or contradictory text immediately. Small edits that do not change behavior or contracts may leave docs unchanged, but the DOX pass still must happen.

## Hierarchy

- Root AGENTS.md is the DOX rail: project-wide instructions, global preferences, durable workflow rules, and the top-level Child DOX Index.
- Child AGENTS.md files own domain-specific instructions and their own Child DOX Index.
- Each parent explains what its direct children cover and what stays owned by the parent.
- The closer a doc is to the work, the more specific and practical it must be.

## Child Doc Shape

- Create a child AGENTS.md when a folder becomes a durable boundary with its own purpose, rules, responsibilities, workflow, materials, or quality standards.
- Work Guidance must reflect the current standards of the project or user instructions; if there are no specific standards or instructions yet, leave it empty.
- Verification must reflect an existing check; if no verification framework exists yet, leave it empty and update it when one exists.

Default section order:

- Purpose
- Ownership
- Local Contracts
- Work Guidance
- Verification
- Child DOX Index

## Style

- Keep docs concise, current, and operational.
- Document stable contracts, not diary entries.
- Put broad rules in parent docs and concrete details in child docs.
- Prefer direct bullets with explicit names.
- Do not duplicate rules across many files unless each scope needs a local version.
- Delete stale notes instead of explaining history.
- Trim obvious statements, repeated rules, misplaced detail, and warnings for risks that no longer exist.

## Closeout

1. Re-check changed paths against the DOX chain.
2. Update nearest owning docs and any affected parents or children.
3. Refresh every affected Child DOX Index.
4. Remove stale or contradictory text.
5. Run existing verification when relevant.
6. Report any docs intentionally left unchanged and why.

## User Preferences

When the user requests a durable behavior change, record it here or in the relevant child AGENTS.md.

- Build the documented RELAY experience across the remaining roadmap phases on one development line before the integrated acceptance run. Use phase order to sequence implementation; during construction run only checks needed to keep the build working. Do not produce a closure review or synthetic acceptance claim for every slice.
- Build one Diagnostics/Debug section into the product as the workflows are implemented. At the end, one integrated run must report workflow outcomes, failures, reproducible privacy-safe logs, component versions, timing, resource use, and evidence limitations. If UEFN, another creator app, or a required hardware tier is unavailable, report its workflow UNTESTED; a synthetic substitute cannot pass that workflow.
- Consolidate useful draft work into the active development line. Do not create more preview releases or review paperwork during construction. Keep the existing binary draft unpublished and leave `main` unmerged until the user changes that direction.
- RELAY source uses the MIT license and credits RAiiNMAN as creator/copyright holder.

- RELAY must include a dashboard as a first-class interface for project status, observability, approvals, history, diagnostics, tests, assets, and integrations.
- The dashboard must never be the only way to perform an operation. Headless CLI execution remains canonical, and dashboard, MCP, REST, WebSocket, and AI clients must use the same underlying command system.
- Dashboard language should be plain and readable for someone unfamiliar with MCP or game-development infrastructure, with advanced detail available progressively rather than forced into the primary view.
- RELAY is a reusable product, not a one-project tool. Core architecture, UI, commands, schemas, storage, tests, and documentation must not assume any single project.
- Multi-project operation is first-class: users must be able to add, discover, inspect, switch, archive, and remove projects while keeping each project's state, history, tests, assets, configuration, and permissions isolated.
- Offer one-click discovery of saved local Codex project folders and recent local task working folders during onboarding, while keeping manual folder add available. The finder must be local, read-only, best-effort, and honest about private-metadata dependencies and cloud-only projects; it must not read chat content or account data.
- Keep long discovery results in a compact chooser, make path inputs open a native picker while preserving paste or typing, and let users collapse dashboard sections. Explain the short project-to-scan-to-check flow and name exactly what basic Health checks cover. Present CLI and MCP as separate connection choices, state ordinary ChatGPT chat's local-connection limit honestly, and offer user-initiated launch buttons only for allowlisted detected apps.
- Project-specific names, rules, fixtures, and examples belong in examples, templates, test fixtures, or project configuration, never hard-coded into RELAY core behavior.
- Design for eventual public release from the start: no personal paths, machine-specific assumptions, private repository names, secrets, account identifiers, or user-specific defaults may be embedded in distributable code or documentation.
- UEFN/Fortnite is the initial integration target, not the permanent architectural boundary. Engine and tool integrations must sit behind adapters so additional project types and toolchains can be added without rewriting RELAY core.
- Public users should be able to install RELAY, connect their own supported tools, create or attach a project, and obtain useful results without understanding the original author's projects or environment.
- Cost efficiency is a primary product objective across the entire RELAY project. Architecture and feature decisions must minimize AI token use, paid-model dependence, remote tool calls, cloud infrastructure cost, redundant computation, and unnecessary latency without sacrificing correctness, safety, or recoverability.
- Prefer deterministic local computation over model reasoning whenever a task can be measured, parsed, validated, indexed, filtered, diffed, deduplicated, summarized structurally, or executed by code.
- Prefer CLI plus compact skills as the initial local-AI strategy, but benchmark it against thin/dynamically discovered MCP for each client class. Keep MCP and remote gateways as adapters rather than core business logic.
- Default AI-facing output must be compact, structured, budget-aware, and progressively retrievable. Raw logs, large result sets, screenshots, and historical evidence stay in RELAY storage until specifically needed.
- Reuse cached results, incremental indexes, deltas, result IDs, and affected-only tests instead of repeating full scans or re-sending unchanged context.
- Do not spend an AI call on work that RELAY can complete deterministically. Expensive-model escalation must be optional and justified by task difficulty rather than treated as the default path.
- Cost and usage observability are product features: RELAY should expose per-job and aggregate metrics for model tokens when available, remote calls, local work, cache hits, context bytes/tokens returned, elapsed time, and estimated avoidable work.

## Child DOX Index

- `.github/AGENTS.md` — owns branch-scoped build/review workflows, validation helpers, and public issue intake templates.
- `crates/AGENTS.md` — owns production Rust crates promoted from Phase 1 evidence during Phase 2 and later implementation phases.
- `docs/AGENTS.md` — owns durable product, architecture, research, roadmap, security, UX, and operating documentation under `docs/`.
- `examples/AGENTS.md` — owns installable, project-agnostic examples and templates; its child index routes the UEFN Verse telemetry example.
- `packaging/AGENTS.md` — owns unsigned local Windows staging, side-by-side install/update planning, and the separate future signed-distribution verifier.
- `plugins/AGENTS.md` — owns distributable local AI client plugins and their transport packaging.
- `skills/AGENTS.md` — owns local AI skill packages, generated command references, and thin CLI wrappers under `skills/`.
- `release/AGENTS.md` — owns local release assembly, preview bundle contents, and package verification under `release/`.
- `spikes/AGENTS.md` — owns Phase 1 technical spikes, benchmark harnesses/results, and prototype verification under `spikes/`.
- `validation/AGENTS.md` — owns the one end-of-build integrated run plan and privacy-safe report harness under `validation/`.
