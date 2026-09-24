# Decision Log

Approved decisions captured from project planning. This file records durable outcomes, not discussion history.

## D-001 — Provider-independent core

Status: Approved

RELAY must not depend on a specific AI provider. ChatGPT, Codex, Claude, local models, and future clients are callers.

## D-002 — Multi-project product

Status: Approved

RELAY is not a tool for one map or project. Projects are isolated workspaces and project-specific behavior must not leak into the core.

## D-003 — Public-release posture

Status: Approved

Architecture, configuration, paths, documentation, and packaging decisions must be compatible with eventual public use. Public release timing and license remain undecided.

## D-004 — RELAY Core owns behavior

Status: Approved

Business logic lives once in RELAY Core/Command Bus. Interfaces call it; they do not reimplement it.

## D-005 — CLI/headless is canonical for local execution

Status: Approved

Meaningful operations must be invocable non-interactively. CLI is the preferred local interface for humans, scripts, CI, and AI clients with shell access.

## D-006 — Dashboard is first-class but not exclusive

Status: Approved

The dashboard provides observability, approvals, history, diagnostics, tests, assets, integrations, and usage views. No operation may exist only in the dashboard.

## D-007 — Cost efficiency is a primary objective

Status: Approved

Design must minimize model tokens, paid-model dependence, remote calls, cloud cost, redundant work, and unnecessary latency without sacrificing correctness, safety, or recoverability.

## D-008 — Deterministic work precedes AI work

Status: Approved

If software can measure, parse, validate, index, diff, filter, deduplicate, summarize structurally, or execute a task reliably, RELAY should do it locally before using model reasoning.

## D-009 — Compact skills are preferred for local AI

Status: Approved

Local AI clients should use small skills and shell/CLI wrappers rather than a large always-loaded MCP tool catalog.

## D-010 — MCP is a thin compatibility layer

Status: Approved

MCP remains supported where useful, especially for clients that require it, but it is not the foundation of RELAY.

## D-011 — Cloud-only clients use a thin remote path

Status: Approved

Normal cloud ChatGPT cannot be assumed to execute local shell commands. A remote gateway/connector may forward structured work to local relayd without moving core behavior to the cloud.

## D-012 — Full output stays in RELAY

Status: Approved

Raw logs, large result sets, screenshots, telemetry, and historical evidence stay in persistent RELAY storage by default. AI receives compact result envelopes and fetches detail progressively.

## D-013 — Context Compiler

Status: Approved direction

RELAY will compile task-specific context from structured state, history, evidence, summaries, current goal, client capability, and a context/token budget. Context eviction is not data deletion.

## D-014 — Structured exact data is protected from lossy summarization

Status: Approved

IDs, paths, coordinates, versions, numerical measurements, error codes, transaction references, and other precision-critical fields remain exact. Narrative/repeated material may be summarized or deduplicated.

## D-015 — Results are referenceable

Status: Approved

Meaningful operations should return durable result IDs so new chats and clients can retrieve evidence without rerunning work or carrying old conversations.

## D-016 — Transactions, verification, and rollback

Status: Approved

RELAY-controlled writes should record before/after state, verify actual post-write state, and expose rollback where technically possible.

## D-017 — Automation with explicit boundaries

Status: Approved

Observation, analysis, validation, indexing, safe internal repair, and reconnection may be automatic. Destructive operations, publishing, credential changes, and spending require explicit approval. Project-content write policy must be configurable.

## D-018 — UEFN/Fortnite is the first integration

Status: Approved

UEFN/Fortnite is the first production target. Blender and Krita are companion asset integrations. The core remains engine-agnostic.

## D-019 — Runtime instrumentation matters

Status: Approved direction

The first integration should include structured runtime telemetry, probes, gameplay assertions, visual/debug overlays where supported, repeatable captures, and state verification rather than relying only on editor automation.

## D-020 — Usage observability is a product feature

Status: Approved

Jobs and aggregate views should measure model usage when available, remote calls, context sent/returned, local operations, cache/index reuse, elapsed time, and other metrics needed to prove RELAY saves work.

## D-021 — DOX governs repository documentation and future code

Status: Approved

AGENTS.md hierarchy is binding. Durable boundaries require appropriate DOX ownership and index maintenance.
