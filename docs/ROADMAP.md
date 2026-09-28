# RELAY Roadmap

The roadmap orders construction of the complete RELAY experience. Phases 0–2 are closed. Phases 3–11 remain open unless their documented capabilities and final acceptance evidence support closure. Build the remaining workflows on `development/relay-v0.1`, with only checks needed to keep construction working. Diagnostics is part of each workflow. The corrected 2026-09-27 end-of-build local run recorded 14 PASS, 0 FAIL, 0 BLOCKED, and 29 UNTESTED across 43 workflows. Unavailable UEFN, creator-app, and hardware checks remain UNTESTED. Do not publish the held preview binary or merge `main` during construction.

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

Status: Complete — Phase 1 closed on 2026-09-25

Gate: Passed — Phase 0 is closed. Stack choices become durable only after Phase 1 prototypes and benchmarks justify them.

Goal: choose the minimum durable stack based on prototypes, not preference.

Completed prototype evidence:

- Spike 1: Node first proved the per-user host + CLI + structured command round trip.
- Spike 2: Node first proved SQLite project/result/checkpoint durability with hard-kill recovery and degraded damaged-store behavior.
- Spike 3: Evidence Storage Lifecycle compression/deduplication and BLOB-vs-file benchmark.
- Spike 4: Windows indexing benchmark; notifications are hints, reconciliation + changed-only parsing is the least-privilege baseline, and USN reading remains optional/privileged.
- Spike 5: resource coexistence benchmark against a live UEFN editor; foreground-safe deferral is primary, soft Windows QoS remains optional for unavoidable background work, and hard CPU caps are rejected as a routine default.
- Spike 6: dashboard shell parity/resource benchmark; static/HTTP presentation hosted by `relayd` is favored over a separate resident dashboard backend.
- Spike 7: neutral Node-vs-Rust runtime/IPC comparison; Rust materially improved footprint, startup/CLI latency, restart time, and explicit current-user pipe security while preserving protocol interoperability.
- Spike 8: Rust reproduced the schema-1 SQLite durability/recovery contract, passed bidirectional database compatibility with Node, and retained a 92.23% post-storage idle-RSS reduction. D-153 selects Rust + bundled SQLite as the Phase 1 local core foundation.
- Spike 9: one JSON command registry plus a bounded JSON Schema 2020-12 validation profile now drives Rust validation and derived CLI/dashboard/adapter/AI discovery metadata with no new runtime dependency. D-154 selects it as the semantic command-contract source.
- Spike 10: synthetic out-of-process adapter broker/manifest benchmark; D-155 selects command-registry-bound on-demand workers with artifact/provenance checks, timeout/backoff/quarantine, and Windows Job Object lifecycle/resource containment while explicitly rejecting the claim that Job Objects are a complete security sandbox.
- Spike 11: synthetic adversarial Windows sandbox benchmark; D-156 selects AppContainer/process isolation semantics with explicit filesystem grants, default-deny egress, broker-allowlisted capabilities, minimized environment, and outer Job Object limits. The measured experimental processmodel backend remains provisional rather than a release API commitment.
- Spike 12: stable documented AppContainer/LPAC benchmark; D-157 selects mailbox-only direct writes, brokered egress, measured-build allowlisting, and fail-closed unsupported tiers while keeping the experimental backend reference-only.
- Spike 13: packaging/update benchmark; D-158 selects signed per-user side-by-side bundles with verify → stage → atomic activation → schema-aware rollback as the default personal/direct Windows path, while retaining MSIX/App Installer as an optional trusted-signing/Store channel.
- Spike 14: logging/diagnostic benchmark; D-159 selects bounded structured JSONL as the default local diagnostic record, with ETW optional deep tracing and independent diagnostics health surfaced in `status` / `doctor`.
- Phase 1 closure review: PASS. Required stack decisions, CLI/host round trip, persistent result round trip, automated tests, startup/restart baselines, and idle CPU/RAM baselines are present.
- Phase 2 handoff: first production Core slice is now accepted; active work continues under the Phase 2 section.

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

Status: Complete — Phase 2 closed on 2026-09-26 after three accepted production slices and a final 54-test workspace closure run.

Completed promotion evidence:

- First core vertical accepted: shared registry validation, schema-2 SQLite project/result/job/idempotency state, provenance/trust metadata, independent diagnostics health, per-user daemon, canonical CLI transport, graceful/hard restart persistence, damaged-store Degraded behavior, and low-footprint resource verification. Evidence: `PHASE2_FIRST_SLICE_EVIDENCE.md`.
- Second core vertical accepted: permission/effect enforcement, trusted identity/project scope, schema-3 transaction/usage/credential/egress state, idempotent transaction de-duplication, data classification/local-only egress, credential-handle metadata/revocation, schema-1/2 migration, hard-restart persistence, and low-footprint resource verification. Evidence: `PHASE2_SECOND_SLICE_EVIDENCE.md`.
- Third core vertical accepted: generic adapter manifest/broker lifecycle, artifact/component revalidation, registry-bound capabilities, bounded failure/quarantine behavior, qualified AppContainer/LPAC worker isolation, Job Object containment, package-lock concurrency, and measured inactive/invocation cost. Evidence: `PHASE2_THIRD_SLICE_EVIDENCE.md`.
- Phase 2 closure review: PASS. All four published exit criteria pass and the exact accepted production tree passed 54 workspace tests. Evidence: `PHASE2_CLOSURE_REVIEW.md`.

Phase 2 handed off to Phase 3. The current cross-phase status is recorded in the sections below.

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

Status: Active — the generic two-project baseline, schema-5 bounded delta/dependency-edge foundation, provisional targeted hint-update path, schema-6 continuity recovery and idle scheduler, bounded Windows OS watcher, schema-7 project configuration, installed sandboxed parser dispatch, and live parser health have synthetic fixture evidence. The development line also includes an offline parser installer draft and schema-8 reversible archive/irreversible removal tombstones that retain history and leave project files alone. The integrated local run exercised import/index and lifecycle on one workstation, but no supported host tier was declared, so tier resource budgets remain UNTESTED. `PHASE3_CLOSURE_REVIEW.md` remains NOT READY. Prolonged creator-app interference, live parser installation and real-adapter proof remain open. The synthetic measurement preview is held and unpublished.

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

Status: Active foundation — `result.describe` provides a project-scoped metadata-only lookup, and `result.context` selects exact scalar facts from an authorized durable result within a requested byte budget. `context.compile` combines up to eight authorized results for one project, preserves each source ID and freshness metadata, and reports same-kind exact-fact conflicts without choosing a winner. `context.task.compile@1` adds one current project/index/configuration database snapshot and at most four explicitly selected durable project-removal confirmation records to bounded historical result facts. It marks missing or stale index state and leaves result currentness unknown without a generation link; local confirmation records never become verified human approval. Required facts or conflicts that do not fit fail explicitly. An eight-entry in-memory cache reuses identical authorized result-only compilations after every source is reloaded and digest-checked; usage/status counters distinguish actual hits, misses, and skipped fact collection. Its runtime benefit has not been measured and it makes no token-savings claim. A deterministic Context Gauntlet fixture measures exact retention and serialized bytes for stored results. The integrated local run measured two tiny stored payloads at 65 JSON bytes versus 1,164 bytes for the compiled context envelope, so this case shows overhead rather than savings. Model quality, token savings, and remote cost remain UNTESTED. General decision memory, source-to-generation linkage, documentation retrieval, and Phase 4 exit criteria remain open. Earlier evidence: `PHASE4_RESULT_DESCRIPTION_EVIDENCE.md`, `PHASE4_CONTEXT_FIRST_SLICE_EVIDENCE.md`, and `PHASE4_CONTEXT_GAUNTLET_FIRST_FIXTURE.md`.

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

Status: Active foundation — the daemon now serves a responsive purple-and-gold local workspace launched from the Windows Start menu. It guides project add/select/index steps, surfaces bounded local tool discovery and truthful UEFN/Blender/Krita readiness, and groups advanced controls behind progressive disclosure. The same command system as the CLI supplies health, project lifecycle and index actions, Diagnostics/Debug, safe support download, stored result/job descriptions, transaction activity, usage, static UEFN inspection, asset checks, and declared check planning/execution. On the installed build, a project was added and indexed through the browser, then two direct indexed-file assertions were planned and passed with durable result IDs. This proves the local browser/engine path only. Project removal retains its schema-10 expiring local confirmation workflow; this is not proof of a human at the keyboard. Shared pause/resume controls expose watcher mode in status/diagnostics. Live UEFN and native creator-app outcomes remain UNTESTED. Manual assistive-technology and integrated security review remain open.

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

Status: Active early foundation — a read-only static inspector classifies a registered project's ready index for top-level `.uefnproject` markers, Verse source names, and Unreal asset/map counts through one shared CLI/dashboard command. `relay uefn-audit` can store that static observation by durable result ID through the shared result command. A bounded localhost MCP client summarizes advertised toolsets and parameter shapes. `uefn.editor.inspect` returns authorized static counts alongside independent local MCP discovery, with editor identity and project binding explicitly unverified. A private invocation primitive validates a freshly advertised tool schema, exact caller arguments, and a declared effect before dispatch; it is not exposed as an authorized host command. Entity/device/spawn observations, native audit, capture, and all live workflow acceptance remain UNTESTED/open.

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

Status: Active early foundation — a bounded parser normalizes structured Verse log fragments and evaluates project-defined local event-count assertions through a shared command. Imported analysis can be recorded by result ID. The daemon can also read and hash a bounded project-local `.log` or `.jsonl` file through an authorized shared command, returning compact analysis; the CLI can store that analysis by result ID. This establishes local-file acquisition, not a verified UEFN session. A project-installable Verse logging template emits session boundaries and one device-ready probe using documented Epic APIs, but has not been compiled or run in UEFN. An isolated runtime model tracks session transitions, probe eligibility, and capture completeness without promoting caller-declared provenance. Live UEFN assertion status remains UNTESTED; editor-bound runtime acquisition, visual capture, and durable live gameplay-test results remain open.

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

Status: Active early foundation — a local asset manifest validator checks bounded source/export links and lineage through CLI/dashboard. The dashboard accepts a bounded local manifest and presents compact findings and lineage counts. Shared `assets.impact.analyze` maps changed project-relative paths to affected declared assets and related dependencies without exposing paths, including files now missing or stale. Krita declared formats can be inspected through the same paths. The bounded Blender headless mesh checker is routed through an authorized shared command and CLI for project-relative `.blend` files; `relay blender-mesh-record` can store its compact outcome by durable result ID without turning an unavailable native check into a pass. Native Blender workflow remains untested until a real installation and file are exercised. A fixed CLI-only Krita command can export one project-local `.kra` to a new `.png` through Krita's documented command line, with bounded process and create-only publication. Successful exports write a project-scoped shared Result Store build record containing source/output SHA-256 and hashed relative identities; the CLI returns its result ID. A record failure after publication is explicit. `relay krita-reconcile` now measures an existing project-local KRA/PNG pair after a possible crash window and durably stores a replayable recovery candidate without altering the PNG. File presence and hashes cannot prove Krita made that output, so its native origin remains unverified and native workflow status remains UNTESTED; a fresh export to a new path is required for native proof. Native Krita execution, UEFN asset integration, broader asset build records, and integrated acceptance remain UNTESTED/open until real app sessions are available.

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

Status: Active early foundation — a compact local `relay-core` skill, command reference generated from the shared registry, and version/capability-checking PowerShell wrapper exist. A runnable authenticated loopback gateway exposes bounded registry discovery and project-scoped result list/describe/context reads with a protected per-user token. The integrated local run exercised discovery and all three result reads over loopback MCP. Its local `/mcp` endpoint implements the exact stateless `2026-07-28` subset; older handshake MCP clients are explicitly unsupported. `remote_client_status: untested`. A live coding-agent workflow, external client token handoff, public remote access, ChatGPT-compatible client, and AI context-overhead comparison remain open.

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

Status: Active foundation — `relay discover` performs a bounded read-only first-run scan for nearby project markers and candidate tool installations while labeling every live integration untested. The dashboard can also find saved local Codex project folders on demand from Codex's private local cache and add the chosen folder through the shared project command; this is a best-effort convenience, not a live Codex API connection. `relay onboard <project-folder>` imports the selected folder and builds its first index through shared commands, with partial-import errors explicit. The dashboard guides add → select → capability check → local index build and gives bounded recovery guidance. Shared `automation.pause`/`automation.resume` commands and `relay pause`/`relay resume` suspend or resume optional watcher work for one daemon run; status and diagnostics expose the mode. `relay support-bundle` exports bounded health/count summaries and can reduce a supplied integrated report to checked counts and resource totals; without a real run, that field stays empty. A revisioned per-project check catalog and generation-atomic planner propose declared checks from change and parser coverage records. Optional per-check direct mode also selects changed declared paths and the first baseline without requiring a parser; omitted mode retains guarded transitive planning. The additive `automation.checks.execute` path runs only allowlisted indexed-file presence/digest assertions from an exact selective plan against a guarded current index snapshot, then atomically records bounded result IDs and a replay-safe job. Undeclared/native checks remain untested with no result ID; incomplete transitive coverage, continuity, or inventory prevents assertion execution. This is indexed-state evidence, not live filesystem or creator-app verification. Automatic tool onboarding, reconnection, general test execution, and live affected-only creator workflows remain open.

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

Status: Open — MIT license and RAiiNMAN credit apply to first-party RELAY source and documentation; the public binary remains unpublished. An unsigned local Windows x64 ZIP now contains install/uninstall entrypoints and a Start menu launcher for the dashboard and engine. The package was installed on this workstation, activated with schema-aware checks, launched, and used for the browser add/index/plan/run path. A separate signed-catalog verifier contract requires exact SHA-256 payload/catalog matching, CA-trusted publisher identity, pinned signer/root certificates, online revocation, and timestamped Windows Authenticode verification; no certificate or real signed package is available to pass it. Clean-account installation, low-tier hardware, signed distribution, and actual public acceptance remain open. The corrected local run has no automated failures but 29 UNTESTED workflows, so it does not establish public acceptance.

Before public beta:

- installer/update path
- public onboarding/time-to-first-value acceptance test
- configuration/support-profile documentation
- migration testing
- security/threat review
- privacy/data-retention docs
- remote-processor/provider policy documentation
- local-only/private-mode verification
- MIT license and RAiiNMAN creator credit selected; verify inclusion in final distribution
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
