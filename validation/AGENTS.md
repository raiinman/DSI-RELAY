# Purpose

Own the one end-of-build integrated validation runner and its bounded workflow plan.

# Ownership

- Governs `validation/` scripts and plan data. Production command behavior stays in the shared registry, Core, CLI, and daemon.
- The runner invokes `relay exec --stdin` against a real private daemon and assembles a privacy-safe report compatible with the `relay-validation` schema. It does not itself certify unavailable creator applications or hardware.

# Local Contracts

- Run the ordered plan once at the end of construction, not as per-slice closure. Do not run it while building the harness.
- A planned workflow is `passed`, `failed`, `blocked`, or `untested`; absent UEFN, creator app, required hardware, project input, or live evidence remains `untested`.
- Observe only registered shared commands. Synthetic fixtures never satisfy live workflows.
- Keep report and journal to fixed codes, opaque scenario IDs, versions, timing, resource counters, and SHA-256 journal references. Do not retain raw command output, arguments, local paths, names, logs, or secrets.
- Reserve a new report and sidecar journal path before launching; never overwrite existing output. Generated evidence is local and ignored by Git.
- Bound child processes. Keep disposable daemon state in a current-user-only temporary directory, reject reparse points before recursive cleanup, and safely collect owned orphans on the next invocation.
- The private daemon state is disposable; project source files are read but not modified by the plan.
- A newly watched project can be stale after baseline creation. The runner may use the shared full-content reconciliation command to establish a ready current index before dependent workflows; it must keep the earlier and current generations distinct when interpreting results.
- The Phase 9 local discovery measurement compares one bounded `registry.list` JSON exchange through `relay exec --stdin` with the same query through the loopback MCP gateway. `transport_metrics` records only application JSON request/response byte counts and elapsed milliseconds; it excludes HTTP headers, bearer tokens, gateway startup, and any claim about AI token savings or remote ChatGPT compatibility. Missing or busy gateway remains `untested`.
- The Phase 7 project-file capture workflow requires an explicitly supplied project-relative log path and matching session ID. It observes only bounded local file acquisition and analysis; absent input is `untested`, and a local pass never certifies a live UEFN session.
- The Phase 4 context benchmark reuses the two stored local results, sums their payload JSON byte counts, and compares that sum with the compiled result JSON byte count using the same PowerShell UTF-8 serializer. It records the observed ratio and separate CLI round-trip elapsed times. A larger compiled result is still a valid measurement. Model tokens, answer quality, and remote cost remain untested.
- The Phase 4 task snapshot scenario compiles current project/index state, two selected result facts, and one disposable rejected removal plan through `context.task.compile`. A watched index may truthfully become stale after the earlier baseline; accept a coherent ready or stale snapshot with an indexed generation. The local decision is automated, so it verifies durable state labeling and exact facts without claiming human approval.
- The project lifecycle scenario tests local approval state transitions through shared commands with an automated decision on a disposable project. It does not establish that a person reviewed the removal; dashboard approval review is a separate manual workflow and stays untested absent direct observation.
- The optional Phase 3 supported-host resource workflow requires an explicitly declared minimum/recommended candidate tier and observed host core/RAM counts. A one-run index command is sampled at 100 ms intervals for daemon/CLI CPU time and Windows working set (resident-memory proxy); the report includes sample/probe overhead and a fixed-code foreground creator-app presence observation. Brief spikes can fall between samples, and elapsed time includes CLI launch and instrumentation. Its PASS means a local measurement was captured, while support-tier budgets remain `untested`. An absent tier, missing host probe, or clear core/RAM mismatch is `untested`. The separate prolonged paired creator-app interference workflow stays `untested` until real interactive traces exist; process presence is not interference proof. This does not replace the minimum-PC manual gate or the repeated Phase 3 budget protocol.
- The Phase 10 check-plan scenario reads an existing catalog and verifies project/catalog generation and `planned_not_run` results without changing the creator's catalog. The disposable private host normally has no catalog, so that workflow stays `untested` with an explicit reason; a no-delta plan also stays `untested` for affected-only behavior. It never writes a fixture change to project content.
- The Phase 10 declared-index-check scenario uses an existing catalog and current selective plan, then verifies durable result IDs and truthful counts. It remains `untested` without a catalog, changed generation, or executable index assertions. A pass covers only declared assertions over the guarded index snapshot, never native creator-app test execution.
- The separate Phase 10 direct-index fixture creates two tiny files only inside the owned disposable run root, registers a direct-only catalog, checks both baseline assertions, changes one file, reconciles, and verifies that only the affected check executes with durable indexed-snapshot evidence. Its PASS is a local generic algorithm/transport check; it does not certify UEFN, creator-app, or the supplied project.
- The Phase 8 Krita recovery fixture uses an unverified local file pair to check durable candidate and replay handling. Native Krita origin and workflow remain untested.
- The Phase 9 local gateway result scenario uses authenticated loopback MCP list, describe, and context tools to verify project-scoped metadata and one bounded exact stored fact by result ID. Its one `local_mcp_http` transport metric sums application JSON bytes and elapsed request time across those three calls. A local pass proves these local transport paths only; a remote or ChatGPT client remains `untested` without its own live session.
- The evidence-capture workflow checks the bounded safe recent diagnostic event projection for count, serialized size, and absence of the run and selected project roots. It never reads raw diagnostic log lines into the integrated report.

# Work Guidance

- Keep the plan explicit and deterministic. Manual workflows have no automated success observation.
- The PowerShell assembler emits the `relay-validation` version 1 report shape; direct invocation of that Rust library remains a future integration point. Preserve failures from attempted commands even if a declared environment later becomes unavailable.

# Verification

- Parse the PowerShell script and JSON plan without executing the integrated run.
- At the end of construction, execute one real run and inspect its privacy and outcome evidence before any release claim.

# Child DOX Index

- No child DOX files currently exist.
