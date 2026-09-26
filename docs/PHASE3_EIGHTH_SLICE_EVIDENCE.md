# Phase 3 generic dependency parser contract evidence

Status: PASS for the synthetic sandboxed parser operation, 2026-09-26. Automatic project dispatch remains open.

## Contract

The shared registry adds adapter-only version-1 `adapter.dependencies.parse`. The broker invokes it through the existing digest-verified, isolated worker path with a bounded UTF-8 source payload. Its input identifies one project-relative source path, current source SHA-256, and generic project type. Its result is an observation: the same source identity plus candidate project-relative targets. The operation is read-only and cannot directly write Core dependency edges.

`AdapterBroker::invoke_dependency_parser` caps source content at 1 MiB, checks the digest shape, uses the installed manifest command binding, then validates the sandboxed worker response. The response must echo the exact source path/hash and contain at most 500 distinct targets. Absolute, escaping, self-referential, and malformed target paths fail. The existing broker still checks worker identity, version, PID, capability set, declared command result schema, sandbox launch, and worker artifact digest. Accepted observations retain the adapter invocation provenance and Job Object limits.

Core remains the only writer of derived dependency edges. The separate `project.dependencies.replace` command still checks project scope, ready index state, expected generation, indexed source digest, and indexed target paths before committing. Adapter metadata and parsed content do not grant that authority.

## Verification

- A live synthetic worker in the qualified Windows AppContainer/LPAC sandbox parsed a generic JSON dependency fixture through the new adapter-only operation. The broker retained installed adapter provenance and one-process Job Object evidence.
- The same fixture rejected a source-identity mismatch, a traversal target, and a traversal input path. Existing adapter tests continue to cover artifact integrity, permissions, network/path/process denial, crash/hang handling, and quarantine.
- The full workspace suite passed 80 active tests; the optimized workspace release build passed. The ignored 15,000-file benchmark was not rerun for this parser-contract change.

## Remaining gate

The daemon does not yet install/select a parser from the declared project adapter binding, authorize sending a source file to that parser, or schedule extraction after an index change. The 1 MiB UTF-8 contract intentionally leaves binary and larger source formats for later adapter-specific negotiation. Automatic dependency-edge extraction, invalidation/reparse after changes, multi-project isolation under installed parsers, and resource-tier evidence remain open Phase 3 gates.
