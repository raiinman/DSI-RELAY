# RELAY Roadmap

The roadmap is ordered to prove cost and architecture fundamentals before building a large integration surface.

## Phase 0 — Documentation baseline and adversarial evidence review

Status: Complete — Phase 0 closed on 2026-09-24

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
- local execution economics review
- index continuity and storage recovery review
- Windows per-user host review
- third-party adapter isolation, provenance, permission, compatibility, and distribution review
- project-data classification, remote-egress, credential-broker, embedding-privacy, and local-only-mode review
- team/workspace identity, delegated-authority, revocation, and concurrency review
- crash recovery, durable-job, restore-test, migration, and outage review
- observability truth, sampling, causality, detector quality, and instrumentation-overhead review
- simplicity, configuration-space, time-to-first-value, and operability review
- public-release legal/licensing, platform-terms, branding, asset-rights, and privacy-claims review
- public contract versioning, schema evolution, deprecation, version-skew, and compatibility-lifecycle review
- performance/resource economics, foreground-interference, indexing-scale, storage-maintenance, and hardware-tier review

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
- file-change tracking has a documented reconciliation path
- local model execution is benchmarked rather than assumed cheaper
- the Windows background host model is compatible with interactive tool integrations
- stable/preview/experimental contracts have explicit compatibility/deprecation promises
- mixed-version behavior is defined for CLI/host/gateway/adapters/companions
- schema evolution rules and historical-result interpretation are documented
- compatibility shims/feature flags have retirement criteria
- adapter architecture is out-of-process by default and has a conceptual capability manifest, provenance model, and quarantine behavior
- remote processing is gated by explicit project/destination data policy
- credentials remain outside model-visible context
- derived embeddings/summaries/captures inherit sensitivity by default
- local-only/private mode has testable egress semantics
- important team writes/approvals carry stale-state preconditions
- agent/client actions preserve useful actor/delegator attribution
- revocation/offboarding semantics cover active and queued work
- foreground creator workload outranks optional background/local-AI work
- basic useful readiness does not require full deep/semantic indexing
- inactive-project idle resource cost is benchmarked
- local-AI routing includes GPU/VRAM/foreground contention
- hardware-tier and long-run resource benchmarks are defined
- resource regressions can fail milestone/release gates

## Phase 1 — Technical spike and stack selection

Status: Active

Gate: Passed — Phase 0 is closed. Stack choices become durable only after Phase 1 prototypes and benchmarks justify them.

Goal: choose the minimum durable stack based on prototypes, not preference.

Completed prototype evidence:

- Spike 1: per-user host + CLI + structured command round trip; Node/runtime and named-pipe security remain provisional.
- Spike 2: SQLite project/result/checkpoint durability with hard-kill recovery and degraded damaged-store behavior.
- Spike 3: Evidence Storage Lifecycle compression/deduplication and BLOB-vs-file benchmark.
- Spike 4: Windows indexing benchmark; notifications are hints, reconciliation + changed-only parsing is the least-privilege baseline, and USN reading remains optional/privileged.
- Spike 5: resource coexistence benchmark against a live UEFN editor; foreground-safe deferral is primary, soft Windows QoS remains optional for unavoidable background work, and hard CPU caps are rejected as a routine default.
- Spike 6: dashboard shell parity/resource benchmark; static/HTTP presentation hosted by `relayd` is favored over a separate resident dashboard backend.
- Spike 7: neutral Node-vs-Rust runtime/IPC comparison; Rust becomes the preferred candidate for deeper parity after materially improving host/dashboard footprint, startup/CLI latency, restart time, and explicit current-user pipe security while preserving protocol interoperability.
- Current next work: Spike 8 — Rust operational-state parity + dependency economics. Node remains the working reference/fallback until the Rust candidate proves the durable SQLite contracts and their package/build cost.

Research/prototype:

- implementation language/runtime
- per-user local host and IPC model
- local persistent storage, reconciliation, integrity, and recovery
- filesystem/watch strategy
- dashboard framework
- progressive-depth/personal-first UX prototype
- command/schema library
- schema/IDL and compatibility-check tooling
- protocol/capability negotiation model
- rolling-upgrade/version-skew test harness
- packaging/service installation
- logging/diagnostics
- local privilege/sandbox model
- client/agent identity and delegated authorization model
- workspace/project role-plus-attribute policy prototype
- project revision/conflict-detection prototype
- revocation propagation tests
- evidence-quality/observability pipeline prototype
- sampling/completeness metadata prototype
- instrumentation-overhead benchmark
- personal golden-path/time-to-first-value benchmark
- configuration-space inventory and supported-profile prototype
- relay doctor/self-diagnostic prototype
- feature-complexity/retirement review
- dependency/license inventory prototype
- companion-license boundary review
- Epic/UEFN terms compatibility register prototype
- contract/stability inventory prototype
- schema compatibility checker prototype
- mixed-version/skew test matrix
- deprecation/migration metadata prototype
- asset provenance/license metadata prototype
- durable job/unknown-outcome prototype
- crash/fault-injection harness
- backup/restore and migration-recovery prototype
- secure update/supply-chain assumptions
- adapter broker/worker isolation prototype
- adapter manifest and compatibility-contract prototype
- data-classification and sensitivity-propagation prototype
- credential-broker prototype
- egress-policy/provider-profile prototype
- local-only network-behavior test harness
- Windows QoS/Job Object resource-control prototype
- USN-assisted indexing benchmark
- foreground-interference/local-AI coexistence benchmark
- storage/index maintenance benchmark
- Evidence Storage Lifecycle compression/deduplication benchmark
- hardware-tier performance fixture definition
- long-run resource-aging/soak harness

Exit criteria:

- architecture decision record for stack
- hello-world relayd + relay CLI
- structured command round trip
- persistent result round trip
- basic automated tests
- cold/warm startup and first-use performance baseline
- idle CPU/RAM baseline with one and multiple projects

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
- data-classification and egress-policy foundation
- credential-handle/broker foundation
- outbound-processing ledger foundation
- agent/client identity attribution foundation
- adapter broker and worker lifecycle foundation
- adapter manifest/capability enforcement foundation
- per-user local host
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
- contract stability/version metadata foundation
- version/capability negotiation foundation

Exit criteria:

- multiple projects coexist without state leakage
- changed-only index update demonstrated
- project capability report works
- full rescan is not required for ordinary small changes
- changed-file processing meets resource/latency budget on supported hardware tiers
- inactive projects stay within idle resource budget

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
- token savings are reported beside local CPU/GPU/RAM/storage and human-latency costs

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
- common personal workflows do not require team/enterprise concepts
- Advanced is not required for normal supported tasks
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
- deprecated-contract usage/local compatibility scan

Exit criteria:

- clean-machine onboarding test
- common integration failure self-recovers or explains one action
- support bundle contains useful diagnostics and no test secrets

## Phase 11 — Public hardening

Before public beta:

- installer/update path
- public onboarding/time-to-first-value acceptance test
- configuration/support-profile documentation
- migration testing
- security/threat review
- privacy/data-retention docs
- remote-processor/provider policy documentation
- local-only/private-mode verification
- license decision
- first-party companion/plugin license review
- dependency/license notice inventory
- Epic/UEFN/Fortnite current-terms and branding review
- privacy/telemetry notice review
- AI-generated-content claims review
- contribution policy
- telemetry policy
- accessibility review
- crash recovery
- compatibility matrix
- documented support/version-skew policy
- stable API/CLI/adapter SDK deprecation policy
- historical-result/schema support policy
- public sample projects/fixtures
- release channels
- legal review of unresolved high-impact distribution questions
- third-party adapter SDK/conformance tests
- adapter artifact/provenance/dependency policy
- curated/local installation workflow before any open marketplace

Exit criteria:

- new user installs without original developer environment
- no private/personal defaults
- upgrade/downgrade/migration scenarios documented
- security/public support requirements met
- third-party adapters cannot execute inside RELAY Core by default
- incompatible adapters fail closed
- adapter install/update permissions and provenance are visible to users

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
