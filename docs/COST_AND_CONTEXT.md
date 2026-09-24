# Cost and Context Architecture

## Objective

RELAY exists in part to make AI-assisted development cheaper.

The system should minimize:

- tokens sent to models
- tokens returned from models
- paid/high-end model dependence
- remote tool calls
- cloud infrastructure work
- repeated scans
- repeated retrieval
- redundant tests
- repeated context
- latency caused by unnecessary AI/tool loops

Correctness, safety, and recoverability are not traded away for lower cost.

## Preferred execution order

When several approaches can solve the same problem, prefer:

1. deterministic local computation
2. cached/indexed result
3. incremental or delta computation
4. local CLI operation
5. compact skill guidance
6. small/cheap AI reasoning
7. remote gateway
8. MCP/other compatibility transport when required
9. expensive model reasoning only when the task genuinely benefits from it

This is a decision heuristic, not a rigid call stack.

## Core principle

Do not spend intelligence on computation.

If RELAY can measure, parse, index, diff, validate, filter, deduplicate, classify by explicit rules, or execute an operation deterministically, it should do that before asking a model.

## Result Store

Large outputs belong in RELAY, not chat.

Examples:

- raw logs
- scan inventories
- screenshots
- telemetry
- profiler output
- test details
- asset reports
- dependency graphs
- command output
- transaction evidence

A caller receives a compact result envelope with a durable result ID.

## Progressive detail levels

Results should support progressive retrieval. The exact API is open, but the semantic levels are:

- Status — success/failure and blockers.
- Summary — small explanation and counts.
- Findings — actionable issues.
- Evidence — measurements, relevant log excerpts, related entities/files.
- Raw — complete stored output where available.

Normal AI use should stop at the lowest level sufficient for the next correct decision.

## Context Compiler

The Context Compiler builds a task-specific package from:

- current goal
- selected project
- current state
- relevant historical events
- relevant results
- user/approved decisions
- exact structured facts
- documentation/version context
- recent interactions where useful
- client capabilities
- token/context budget

It should maximize:

- relevance
- factual precision
- current-state accuracy
- necessary evidence coverage

while minimizing:

- duplication
- stale information
- irrelevant history
- token count

## Memory tiers

RELAY should separate:

### Active context

Small working set for the current AI task.

### Structured project memory

Current normalized state, decisions, entities, dependencies, assets, tests, results, and transactions.

### Historical/event memory

Durable history of changes and important events. Current state must not erase useful history.

### Raw evidence

Original logs, screenshots, captures, telemetry, command output, and source artifacts.

The AI may forget an item from active context without RELAY deleting it.

## Exact versus lossy information

Never casually paraphrase precision-critical data.

Keep exact:

- identifiers
- paths and filenames
- coordinates/transforms
- versions
- numerical measurements
- timestamps when relevant
- error codes
- transaction IDs
- result IDs
- command arguments
- permission decisions

May be summarized/deduplicated:

- repeated warnings
- verbose narrative logs
- descriptive history
- large groups of successful checks
- unchanged inventory
- low-priority background events

## Semantic chunking

Never split structured evidence merely because of an arbitrary token boundary.

Chunk around natural units such as:

- function
- file section
- stack trace
- transaction
- test case
- entity finding
- log incident
- document section
- asset record

## Filtering and deduplication

Default AI result processing should:

- collapse repeated identical errors
- group equivalent findings
- aggregate passes
- prioritize failures and warnings
- retain first/last occurrence for repeated incidents
- preserve references to full evidence

## Deltas

After a baseline exists, prefer changes since the relevant prior state.

Examples:

- changed files
- added/removed entities
- new test failures
- altered assets
- changed integration version
- new runtime errors

Do not re-send an entire project inventory when only three facts changed.

## Affected-only work

Map changes to the checks they can affect.

Examples:

- spawn transform change -> spawn clearance, sightline, spawn capture
- Verse progression change -> compile, progression tests
- texture change -> dimensions, format, memory-related asset checks

Full suites remain available on demand and at important gates.

## Screenshots and visual evidence

Capture may generate many images. Store all required evidence, but send only materially relevant images to AI unless the caller requests more.

Local comparison should identify unchanged/minor/significant differences where technically practical.

## Context budgets

AI-facing operations should accept a budget or client profile.

A budget controls the amount of derived context returned, not the amount of deterministic work RELAY may perform locally.

If content exceeds the budget:

- preserve critical facts
- return references
- state that more detail exists
- never silently truncate an important finding

## Usage observability

Every meaningful job should record what can be measured, including:

- local operations
- cache/index hits
- remote calls
- AI calls
- model/token usage when available
- context bytes/tokens returned
- raw result size
- elapsed time
- retries
- tests avoided/reused where measurable

Aggregate views should help answer whether RELAY is actually reducing usage.

## Context Gauntlet

RELAY must benchmark context strategies rather than assume they work.

Compare, where practical:

- full history
- recent-only
- simple summary
- retrieval
- hierarchical retrieval
- RELAY Context Compiler

Test difficult cases:

- critical fact near beginning/middle/end
- old state contradicted by new state
- thousands of repeated warnings
- one exact coordinate hidden in noise
- multi-hop dependency questions
- changing task focus
- large raw logs with few relevant lines

Measure:

- task success
- exact fact retention
- retrieval recall
- hallucination/wrong-state rate
- tokens
- latency
- model/tool calls
- cost when measurable

See RESEARCH_PLAN.md for the research basis and evaluation backlog.
