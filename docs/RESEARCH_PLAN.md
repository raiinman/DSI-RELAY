# RELAY Research Plan

## Purpose

RELAY should be based on measured behavior rather than assumptions about AI context, MCP, editor automation, or runtime integrations.

Research outputs must feed specific architecture decisions and benchmarks.

## Research track A — Long context and context compilation

Questions:

- How much context is actually useful for debugging/build tasks?
- How often does full history hurt retrieval or decision quality?
- Which facts must remain exact?
- Which summaries can be lossy?
- Does hierarchical retrieval outperform flat retrieval for project state?
- How should context budgets change across small and large models?
- How do task changes affect prior compacted context?

Initial academic references:

1. Liu et al., "Lost in the Middle: How Language Models Use Long Contexts"
   - https://arxiv.org/abs/2307.03172
   - Relevant to position sensitivity and the danger of assuming that fitting information into a context window means the model will use it reliably.

2. Packer et al., "MemGPT: Towards LLMs as Operating Systems"
   - https://arxiv.org/abs/2310.08560
   - Relevant to tiered/virtual context management and external memory.

3. Sarthi et al., "RAPTOR: Recursive Abstractive Processing for Tree-Organized Retrieval"
   - https://arxiv.org/abs/2401.18059
   - Relevant to hierarchical summaries and retrieval at multiple abstraction levels.

4. Jiang et al., "LLMLingua: Compressing Prompts for Accelerated Inference of Large Language Models"
   - https://aclanthology.org/2023.emnlp-main.825/
   - Relevant to prompt compression and explicit token budgets.

5. Jiang et al., "LongLLMLingua: Accelerating and Enhancing LLMs in Long Context Scenarios via Prompt Compression"
   - https://aclanthology.org/2024.acl-long.91/
   - Relevant to long-context compression, information density, cost, and position effects.

6. Li et al., "Compressing Context to Enhance Inference Efficiency of Large Language Models"
   - https://aclanthology.org/2023.emnlp-main.391/
   - Relevant to selective context compression.

These references motivate experiments; they do not automatically dictate RELAY's implementation.

## Context Gauntlet

Create repeatable tasks and compare:

- full conversation/history
- recent window only
- simple summary
- flat retrieval
- hierarchical retrieval
- RELAY Context Compiler

Test scenarios:

- key fact at start/middle/end
- stale old state versus current state
- exact coordinates/IDs mixed into prose
- 500+ duplicate warnings
- large logs with a tiny relevant incident
- multi-hop dependency question
- task focus changes halfway through
- results spanning files + runtime + asset state

Models:

- small/free-tier class where testable
- mid-tier model
- high-capability model

Metrics:

- task success
- exact fact retention
- wrong-state decisions
- hallucination
- retrieval recall/precision
- total input/output tokens
- tool calls
- latency
- monetary cost where available

## Research track B — CLI/skills versus MCP

Design hypothesis:

CLI + small task skills will often use less context and fewer calls than a large MCP catalog.

Test the same workflows using:

1. CLI + compact skill
2. thin MCP with structured execute/result
3. broader domain MCP
4. one-tool-per-operation MCP where available

Measure:

- tool/schema tokens loaded
- execution calls
- retries
- task success
- latency
- model tokens
- ease of recovery
- maintenance complexity

Reddit discussion that triggered this research:
- https://www.reddit.com/r/aigamedev/comments/1v484mi/mcp_is_worse_than_no_mcp_godot/

Treat community posts as hypotheses and experience reports, not authoritative evidence.

## Research track C — UEFN control surfaces

Validate against current official Epic documentation and actual target versions:

- supported UEFN editor automation/control interfaces
- Verse logging/runtime instrumentation
- debug drawing
- play-session control
- screenshot/capture options
- device/entity inspection
- performance/memory interfaces
- allowed runtime networking/instrumentation boundaries
- import/export constraints
- public-release restrictions/validation

Produce a capability matrix by UEFN version.

Do not design core contracts around an undocumented behavior without a fallback.

## Research track D — Runtime observability

Prototype:

- structured log protocol
- probes
- assertions
- stable object references
- runtime event deduplication
- test-state workflows
- fixed captures
- state verification

Measure the smallest telemetry set that reliably answers debugging questions.

## Research track E — Local storage and indexing

Compare candidate storage/index designs for:

- multi-project isolation
- structured state
- append-only history
- result/evidence metadata
- full-text/log search
- dependency graph
- incremental indexing
- migrations
- corruption recovery
- public packaging

Do not select a database purely because it is familiar.

## Research track F — Dashboard usability

Test with users who do not know MCP.

Questions:

- Can they tell whether RELAY is healthy?
- Can they tell what the AI changed?
- Can they find a failed test?
- Can they approve/reject a write safely?
- Can they understand why an integration is unavailable?
- Can they create a diagnostic bundle?

Primary language should remain simple; advanced technical data must remain accessible.

## Research track G — Cost accounting

Define how RELAY estimates and reports savings without fake precision.

Possible metrics:

- local deterministic operations
- bytes/tokens avoided from raw result
- context returned
- cache hit rate
- incremental versus full work
- AI/model calls
- remote calls
- affected tests versus full suite
- elapsed time

"Estimated tokens avoided" must clearly be labeled as an estimate unless measured from an actual alternative run.

## Research track H — Security

Before remote/public use:

- threat model
- structured command injection
- shell/path injection
- malicious project content
- secret leakage in logs/context
- cross-project isolation
- remote replay
- approval bypass
- update supply chain
- diagnostic bundle leakage

## Research output rule

A research task is complete only when it produces one of:

- architecture decision
- rejected option with evidence
- benchmark result
- capability matrix
- implementation requirement
- new unresolved question

Research should not become a pile of links disconnected from product decisions.
