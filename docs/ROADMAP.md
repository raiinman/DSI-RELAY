# RELAY Roadmap

The roadmap is ordered to prove cost and architecture fundamentals before building a large integration surface.

## Phase 0 — Documentation baseline and adversarial evidence review

Status: Reopened — adversarial evidence review in progress

Deliver:

- DOX hierarchy
- README
- product vision
- consolidated project plan
- system architecture
- cost/context architecture
- CLI/skills strategy
- automation/dashboard plan
- UEFN v0.1 plan
- security model
- research plan
- decision log
- adversarial architecture review
- evidence register covering current, historical, government, academic, and first-party platform sources
- revalidation/reclassification of affected decisions
- prompt-injection and agent-identity security requirements
- human-factors review for approval fatigue, alarm fatigue, automation surprise, and out-of-the-loop risk
- evidence-retention lifecycle requirements
- benchmark methodology requirements

Exit criteria:

- no major approved concept exists only in chat
- open decisions are explicitly marked
- implementation phases have acceptance criteria
- PHASE0_ADVERSARIAL_REVIEW.md findings are reflected in owning documents
- affected DECISION_LOG.md entries are reclassified where evidence weakens an assumption
- security model treats project/tool content as untrusted and defines agent/client identity boundaries
- Context Compiler requirements cover provenance, intent, freshness, conflicts, exact fields, and memory quality
- automation/UX requirements address consent fatigue, warning quality, and operator situation awareness
- evidence retention has explicit lifecycle/quota/privacy requirements
- UEFN design is native-tool-first and version/capability-gated
- RELAY benchmarks define measurement targets, baselines, protocol details, uncertainty, and reproducibility requirements
- Phase 1 does not lock a stack before these requirements are testable

## Phase 1 — Technical spike and stack selection

Gate: Phase 0 must be closed again before stack choices become durable architecture decisions.

Goal: choose the minimum durable stack based on prototypes, not preference.

Research/prototype:

- implementation language/runtime
- local daemon/IPC model
- local persistent storage
- filesystem/watch strategy
- dashboard framework
- command/schema library
- packaging/service installation
- logging/diagnostics
- local privilege/sandbox model
- client/agent identity and delegated authorization model
- secure update/supply-chain assumptions

Exit criteria:

- architecture decision record for stack
- hello-world relayd + relay CLI
- structured command round trip
- persistent result round trip
- basic automated tests

## Phase 2 — Core and command system

Build:

- project registry
- command registry
- structured request validation
- jobs
- result envelopes
- persistent result store
- permission categories
- idempotency foundation
- transaction foundation
- usage metrics foundation
- provenance/trust metadata foundation
- agent/client identity attribution foundation
- local daemon
- CLI

Exit criteria:

- same operation callable human-friendly and structured machine mode
- durable result ID works across processes
- no interactive prompt in machine mode
- measured job metadata exists

## Phase 3 — Project discovery and indexing

Build:

- tool discovery foundation
- project discovery/add/import
- project isolation
- incremental watcher/index
- capability model
- baseline state
- change/delta model
- dependency graph foundation
- configuration/migration framework

Exit criteria:

- multiple projects coexist without state leakage
- changed-only index update demonstrated
- project capability report works
- full rescan is not required for ordinary small changes

## Phase 4 — Context and cost engine

Build:

- Context Compiler v1
- progressive result retrieval
- exact-field protection
- deduplication
- severity filtering
- semantic sections
- context budgets
- cached context/result reuse
- usage dashboard metrics
- Context Gauntlet harness

Exit criteria:

- compact result retrieves full evidence by ID
- exact facts survive compilation tests
- full-history versus compiled-context benchmarks run
- context/remote-call savings are measurable

## Phase 5 — Dashboard

Build first usable dashboard:

- project list
- overview/health
- jobs
- findings/results
- approvals
- transactions/history
- integrations
- tests
- assets shell
- usage
- diagnostics/advanced
- pause control

Exit criteria:

- dashboard invokes the same command system as CLI
- no dashboard-only capability
- plain-language primary status
- raw technical detail accessible progressively

## Phase 6 — UEFN static/editor adapter

Build:

- project detection
- UEFN integration status/capabilities
- Verse/source discovery
- editor connection adapter based on validated supported surfaces
- entity/device/spawn inspection
- spawn visualization where supported
- compact UEFN audit v1
- fixed capture foundation

Exit criteria:

- supported UEFN project inspected without AI enumerating it
- spawn workflow works end-to-end
- dependency-unavailable states are accurate
- evidence stored by result ID

## Phase 7 — Fortnite runtime bridge

Build:

- structured Verse telemetry
- runtime parser/normalizer
- probe framework
- assertion/test harness
- runtime sessions/results
- test-state experiments
- debug visualization support where allowed
- capture/regression integration

Exit criteria:

- at least one runtime debugging issue can be diagnosed from compact structured evidence
- at least one project-defined gameplay assertion executes and persists
- AI does not need raw full session logs

## Phase 8 — Asset adapters

Build:

- asset registry/manifest
- Blender discovery and headless workflows
- deterministic mesh validation
- Krita adapter/workflow integration
- source/export relationships
- asset build/validation results
- asset impact checks

Exit criteria:

- asset source -> validation -> export/project record is traceable
- bulk asset evidence stays outside AI context
- failed validation is compactly explainable

## Phase 9 — Skills and remote clients

Build:

- relay-core skill
- UEFN/runtime skills
- generated command reference
- safe wrappers
- skill version verification
- thin remote gateway prototype
- minimal ChatGPT-compatible connector/MCP surface as available
- capability negotiation

Exit criteria:

- local coding agent completes target workflows with skill + CLI
- remote client completes target workflows without direct local shell
- MCP/remote context overhead benchmarked against CLI/skill path

## Phase 10 — Automation and recovery

Build:

- first-run wizard
- automatic tool discovery
- initial baseline
- health monitoring
- safe reconnection/self-repair
- affected-only test automation
- visual regression automation
- queued dependency jobs
- support bundle/redaction

Exit criteria:

- clean-machine onboarding test
- common integration failure self-recovers or explains one action
- support bundle contains useful diagnostics and no test secrets

## Phase 11 — Public hardening

Before public beta:

- installer/update path
- migration testing
- security/threat review
- privacy/data-retention docs
- license decision
- contribution policy
- telemetry policy
- accessibility review
- crash recovery
- compatibility matrix
- public sample projects/fixtures
- release channels

Exit criteria:

- new user installs without original developer environment
- no private/personal defaults
- upgrade/downgrade/migration scenarios documented
- security/public support requirements met

## Phase 12 — Additional engine/tool adapters

Only after core contracts have proven stable.

Candidates should be selected by user demand and adapter feasibility.

The new adapter must reuse:

- project registry
- command system
- jobs/results
- context compiler
- transactions
- usage metrics
- dashboard shell
- security model

If a second engine requires rewriting those systems, the core abstraction is not ready.
