# DOX framework

- DOX is the highly performant AGENTS.md hierarchy installed here.
- Agents must follow DOX instructions across any edits.

## Core Contract

- AGENTS.md files are binding work contracts for their subtrees.
- Work products, source materials, instructions, records, assets, and durable docs must stay understandable from the nearest applicable AGENTS.md plus every parent AGENTS.md above it.

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

## Child DOX Index

- No child DOX files exist yet. The root AGENTS.md currently owns the entire repository.
- Before adding substantive project structure, recursively scan the repository, create child AGENTS.md files for durable boundaries where needed, and replace this entry with the actual direct-child index.
