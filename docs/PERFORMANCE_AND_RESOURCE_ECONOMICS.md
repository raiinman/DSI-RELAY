# Performance and Resource Economics

## Purpose

Keep RELAY cheap in the broader sense: low AI usage, low local overhead, low latency, low storage growth, low power/thermal impact, and minimal interference with the creator's foreground tools.

RELAY should feel like infrastructure that disappears into the background, not a second heavyweight development environment competing with UEFN, Fortnite, Blender, Krita, or local AI workloads.

## Core invariants

- Foreground creative work wins over background RELAY work.
- Local deterministic work is preferred only when its local resource cost is acceptable.
- "Background" means resource-adaptive, not merely hidden.
- Inactive projects should approach near-zero routine CPU/GPU activity.
- Full indexing, compaction, deep validation, high-detail telemetry, and local model inference are schedulable workloads, not always-on requirements.
- Startup should become useful before every deep index is complete.
- Performance claims are measured across realistic hardware/project tiers.
- Tail latency and interactive responsiveness matter more than throughput alone.
- Local AI/model inference is paused/throttled/routed when it would materially harm the active creator workload.
- Storage maintenance must not surprise the user with large I/O spikes or disk-space requirements.
- Resource controls are observed first and hard-capped only when evidence justifies it.
- Performance/resource monitoring must remain cheaper than the problem it measures.

## Resource budget

RELAY should account for:

- CPU utilization
- CPU scheduling priority/QoS
- RAM/working set
- GPU utilization
- VRAM
- memory bandwidth where measurable
- disk read/write throughput and IOPS
- database/index size
- screenshot/evidence storage
- network bandwidth
- background wakeups/polling
- battery/power impact where measurable
- thermal/fan impact where measurable
- foreground latency/frame-time impact

The final budgets are benchmark-driven rather than hard-coded during Phase 0.

## Foreground-first scheduling

The active creator workload is the priority.

Typical foreground states include:

- UEFN actively used
- Fortnite play session active
- Blender/Krita interactive editing
- user actively reviewing the dashboard
- time-sensitive RELAY command requested directly

Typical background work includes:

- deep indexing
- evidence compaction
- historical cleanup
- compatibility scans
- full test suites
- local embedding generation
- optional local AI summarization
- storage maintenance
- inactive-project reconciliation

Background work should yield when foreground contention is detected.

## Windows execution controls

Phase 1 should evaluate Windows mechanisms rather than inventing a custom scheduler first.

Candidate controls include:

- EcoQoS / process or thread power throttling for non-foreground work
- process/thread priority
- memory priority
- Job Objects for CPU/memory/process limits and notifications
- OS/user activity and machine-idle signals
- power/battery/thermal state where accessible

Hard resource caps can themselves cause pathological latency or failures, so use notification/priority/backoff mechanisms unless hard caps are justified by testing.

Phase 1 Spike 5 evidence currently favors deferral/backoff over permanent throttling. With a live UEFN editor on the high-end fixture, Normal-priority heavy background work did not materially move editor message-pump p95 even at full logical-CPU saturation. Below-Normal/EcoQoS remained valid soft signals but reduced background throughput without a measured p95 gain. A 25% Job Object hard cap cut background throughput by more than half and worsened p99 tail latency, so hard caps are not a routine foreground-safety default. These results do not replace the required minimum/recommended hardware and Fortnite play-session benchmarks.

## Workload modes

Conceptual modes:

### Foreground safe

Default while an editor/play session is active.

- light indexing only
- low-detail telemetry
- no optional local AI GPU workload
- maintenance deferred
- inactive projects quiescent

### Balanced

Normal desktop use when foreground pressure is moderate.

- incremental indexing
- ordinary validation
- bounded background tasks
- optional local model work only when headroom exists

### Idle boost

Machine is idle / user explicitly requests batch work.

- deep indexing
- full test suites
- compaction/maintenance
- bulk asset analysis
- optional local model/embedding jobs

### Diagnostic burst

Short, explicit high-detail capture.

- targeted probes
- high-detail telemetry
- fixed time/resource budget
- automatic return to normal mode

These can remain internal policy states rather than user-facing knobs if automation is reliable.

## Startup and readiness

RELAY should not block first use on full indexing.

Startup stages may expose:

1. host/dashboard ready
2. project discovered
3. lightweight metadata/index ready
4. basic local audit ready
5. deep/content index ready
6. optional semantic/vector index ready

The dashboard communicates partial readiness honestly.

A new user should reach a useful deterministic audit before optional deep indexing completes whenever the integration allows it.

## Indexing strategy

Use the cheapest trustworthy layer first.

Conceptual path:

~~~
initial/reconciliation scan
        |
        v
persisted baseline
        |
        +-- filesystem change feed
        +-- NTFS USN journal where available/useful
        |
        v
incremental update
        |
        +-- targeted content parse
        +-- targeted semantic/index work
~~~

Rules:

- file notifications/journals accelerate indexing but do not replace reconciliation
- content indexing is limited to relevant project roots/types
- generated/cache/build folders are excluded by integration rules where safe
- inactive projects avoid full continuous deep indexing
- parsing work is deduplicated by content/version identity where practical
- semantic/vector indexing is optional and policy/resource aware

## Multi-project idle cost

Adding a project must not permanently add a full background stack.

Inactive projects should prefer:

- persisted state on disk
- lightweight change markers
- no loaded semantic index unless queried
- no active adapter worker unless needed
- no local model process
- bounded watcher/journal tracking
- deferred reconciliation

Opening/switching projects can warm necessary state.

## Local model scheduling

Local AI is optional compute, not privileged background work.

Before starting local inference consider:

- active foreground editor/play session
- current GPU/VRAM pressure
- CPU/memory pressure
- local model working-set requirement
- battery/power state
- expected latency
- cloud/private-mode policy

Possible actions:

- run locally now
- queue until idle
- use CPU instead of GPU if measured better
- select smaller local model
- use approved cloud model
- ask user for a performance/privacy trade-off only when needed

Do not evict or degrade foreground creator workloads merely to save remote model tokens.

## GPU and VRAM coexistence

Consumer GPUs can be especially contention-sensitive when multiple large working sets compete for VRAM.

RELAY must benchmark:

- UEFN only
- UEFN + Fortnite session
- UEFN + local LLM
- Blender/Krita + local LLM
- capture/vision/model workloads together

Measure:

- foreground latency/frame time
- VRAM usage
- GPU utilization
- CPU pinned/system memory spill where measurable
- local model throughput
- crash/out-of-memory behavior

Automatic local inference should back off when it harms the foreground workload beyond the supported budget.

## Database and index maintenance

The selected database/index technology needs explicit maintenance economics.

Evaluate:

- steady-state file growth
- tombstones/free space
- compaction/vacuum/checkpoint behavior
- disk-space headroom required for maintenance
- read/write latency during maintenance
- concurrent-reader/writer behavior
- crash recovery after maintenance interruption

Maintenance should run during safe windows when possible and remain bounded by storage/resource policy.

Phase 1 Spike 8 confirms the selected Rust + bundled-SQLite operational-state path preserves its runtime advantage after real durable state is loaded. After hard-kill recovery, Rust sampled 9,367,552 bytes RSS versus 120,500,224 bytes for the Node reference, with zero CPU delta over the five-second idle sample. Result write/read/`quick_check` p50 latency remained near the Node reference; Rust checkpoint updates were slower on this fixture (1.563 ms p50 versus 0.794 ms) but still low in absolute terms. The trade-off moves into build/package cost: the stripped Rust binary grew from 438,272 bytes before SQLite to 2,153,472 bytes with bundled SQLite, and a cached-crate clean release build measured about 32 seconds. Concurrency, WAL pressure, VACUUM, and interrupted maintenance remain separate acceptance gates.

## Evidence and screenshot storage

Evidence can dominate disk use.

Use tiered storage concepts:

- metadata/thumbnail/summary kept longer
- full-resolution captures retained by policy
- duplicate/unchanged captures content-hash deduplicated where safe
- expired telemetry compacted/pruned
- important/pinned evidence protected
- raw evidence stored outside the database when large-file storage is more efficient

Do not store multi-megabyte binary blobs in an embedded database merely because the database can technically hold them; benchmark file/object versus BLOB storage.

## Storage pressure

Maintain headroom for:

- active transactions
- recovery/checkpoints
- database maintenance
- update/migration
- crash diagnostics

A storage quota should account for temporary maintenance amplification, not only steady-state size.

## Polling and wakeups

Avoid periodic high-frequency polling when event-driven or adaptive mechanisms are available.

If polling is required:

- back off when stable
- coalesce related checks
- suspend inactive integrations
- increase frequency only during active jobs
- measure wakeup/CPU/network cost

## Interactive performance metrics

Measure foreground experience with latency-focused metrics, not only aggregate throughput.

Potential metrics:

- dashboard command latency p50/p95/p99
- UEFN/editor interaction latency
- Fortnite frame-time/jank delta where measurable
- Blender/Krita interaction latency
- time to first useful project result
- time to project switch/warm
- time to deep-index completion
- indexing throughput
- query latency
- background CPU/RAM/GPU percentiles
- worst-case maintenance pause

A fast average with ugly tail latency is a failure for an interactive tool.

## Hardware tiers

Benchmark representative tiers anchored to actual supported creator environments.

Initial UEFN-oriented tiers should include at least:

### Minimum-class

Around Epic's minimum UEFN requirements.

### Recommended-class

Around Epic's recommended UEFN requirements.

### High-end creator workstation

Substantially above recommended, suitable for local AI coexistence experiments.

The final hardware fixtures should be explicit/versioned and not depend on one developer's personal PC.

## Project scale tiers

Benchmark projects such as:

- small: prototype/simple island
- medium: production-sized ordinary project
- large: asset-heavy/multi-system project
- stress: intentionally extreme file/entity/evidence counts

Use concrete fixture definitions instead of vague labels in actual benchmark reports.

## Performance admission gate

A new feature or adapter can fail release even when functionally correct if it:

- materially slows startup
- increases idle resource use
- harms UEFN/creator responsiveness
- adds unbounded storage growth
- keeps inactive projects/processes awake
- creates large tail-latency spikes
- increases maintenance I/O beyond budget

Every high-background-cost feature needs an on-demand/deferred alternative.

## Performance regression testing

CI/dogfood performance testing should track trends for:

- cold/warm startup
- project open
- incremental change processing
- audit latency
- query latency
- idle CPU/RAM
- active indexing CPU/RAM/disk
- storage growth
- maintenance duration
- foreground interference
- local model coexistence where applicable

Use pinned fixtures/hardware or normalized comparable environments.

## Performance self-observability

RELAY should be able to answer:

~~~
What is using resources right now?
Why is it running?
Which project/job owns it?
Can it pause?
What will pause if UEFN becomes active?
~~~

A future dashboard/CLI performance view may expose:

- top RELAY workers by CPU/RAM/I/O
- active background jobs
- resource mode
- local AI status
- storage/index sizes
- deferred work
- contention warnings

Do not expose hundreds of low-level counters by default.

## Energy and power

Energy is not assumed to equal monetary cost for every user, but it affects battery, thermals, fan noise, and total compute cost.

Where practical measure:

- wall power or supported OS/hardware proxies
- task energy relative to work completed
- idle versus active draw
- local AI versus approved remote route in controlled comparisons

Government/industry energy reports justify treating compute energy as a material resource, but RELAY must benchmark desktop workloads directly rather than extrapolating data-center totals into local-product claims.

## Open questions

- initial numerical CPU/RAM/disk/GPU budgets
- performance-mode automation signals
- Windows Job Object/EcoQoS implementation
- USN journal support scope
- index storage architecture
- large binary evidence storage format
- local-model scheduler/routing policy
- foreground-app activity detection
- hardware benchmark fixtures
- multi-project watcher strategy
- performance regression infrastructure
- energy measurement method
