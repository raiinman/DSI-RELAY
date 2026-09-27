# Purpose

Own the versioned report model for one integrated end-of-build RELAY validation run.

# Ownership

- Governs `crates/relay-validation/`.
- Accepts workflow plans, environment availability, and observations from an authorized runner; it does not execute tests, read logs, or access creator applications.

# Local Contracts

- Every planned workflow receives one outcome: `passed`, `failed`, `blocked`, or `untested`.
- Missing UEFN, creator app, or required hardware availability yields `untested`. A synthetic observation cannot pass an integrated workflow.
- Preserve explicit reason codes, component versions, timing, resource use, opaque log references, and reproducible scenario identifiers. Never include raw logs, local paths, secrets, or free-form diagnostic text in the report.
- Unknown environment availability, absent observations, and missing live evidence remain visible rather than being counted as passed.
- An attempted workflow failure remains failed even if a required environment later becomes unavailable; an unattempted unavailable workflow remains untested.
- Optional transport metrics name only the CLI stdin or local MCP HTTP path and count bounded application-JSON request/response bytes and elapsed milliseconds. They do not estimate model tokens, remote latency, or money saved.
- The serialized integrated report can be deserialized for bounded local support-summary extraction; deserialization does not establish evidence authenticity.

# Work Guidance

- Keep aggregation deterministic and side-effect free.
- The runner must independently establish that an `observed` evidence reference came from its declared environment.

# Verification

- Run focused crate tests for classification and privacy boundaries.

# Child DOX Index

- No child DOX files currently exist.
