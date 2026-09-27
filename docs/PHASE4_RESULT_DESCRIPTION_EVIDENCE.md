# Result description foundation

Status: implemented on `phase3/project-discovery`; Phase 4 remains open.

`result.describe` reads an existing durable result by opaque ID and returns its identity, project, kind, schema and producer versions, SHA-256 digest, stored UTF-8 payload byte count, trust label, and creation time. It does not select or serialize the payload or provenance. The query uses the existing results table and needs no storage migration. `result.get` remains the explicit full-payload retrieval path.

The command uses the shared registry and Core execution authority. A client scoped to a different project receives `PROJECT_SCOPE_DENIED`, as with `result.get`. A missing ID receives `RESULT_NOT_FOUND`.

Verification: `cargo test -p relay-contracts -p relay-core` passed 9 contract tests and 46 Core tests. The new Core test stores a 65 KiB-class Unicode payload, checks the returned byte count and digest against the stored record, confirms the response contains neither payload nor provenance and stays under 1 KiB, denies a cross-project reader, and repeats the lookup after reopening storage.

This is only the first progressive-retrieval primitive. It does not implement finding-level paging, Context Compiler selection, exact-field protection, context budgets, or measured token savings. Metadata fields inherited from existing result writes are not yet length-limited, so the under-1 KiB observation applies to the test fixture, not every stored result.
