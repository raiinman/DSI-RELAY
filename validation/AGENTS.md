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
- The Phase 9 local discovery measurement compares one bounded `registry.list` JSON exchange through `relay exec --stdin` with the same query through the loopback MCP gateway. `transport_metrics` records only application JSON request/response byte counts and elapsed milliseconds; it excludes HTTP headers, bearer tokens, gateway startup, and any claim about AI token savings or remote ChatGPT compatibility. Missing or busy gateway remains `untested`.
- The Phase 7 project-file capture workflow requires an explicitly supplied project-relative log path and matching session ID. It observes only bounded local file acquisition and analysis; absent input is `untested`, and a local pass never certifies a live UEFN session.
- The Phase 10 check-plan scenario reads an existing catalog and verifies project/catalog generation and `planned_not_run` results without changing the creator's catalog. The disposable private host normally has no catalog, so that workflow stays `untested` with an explicit reason; a no-delta plan also stays `untested` for affected-only behavior. It never writes a fixture change to project content.

# Work Guidance

- Keep the plan explicit and deterministic. Manual workflows have no automated success observation.
- The PowerShell assembler emits the `relay-validation` version 1 report shape; direct invocation of that Rust library remains a future integration point. Preserve failures from attempted commands even if a declared environment later becomes unavailable.

# Verification

- Parse the PowerShell script and JSON plan without executing the integrated run.
- At the end of construction, execute one real run and inspect its privacy and outcome evidence before any release claim.

# Child DOX Index

- No child DOX files currently exist.
