# Phase 3 project configuration/version metadata evidence

Status: PASS for the versioned configuration foundation, 2026-09-26. Automatic dependency extraction remains open.

## Contract

Schema 7 adds a project-scoped `project_configuration` record without changing source files or schema-6 index continuity state. The version-1 `project.configuration.put` and `project.configuration.get` commands use the shared registry, normal Core authority and transaction handling, and the existing CLI/daemon transport. A configuration records a bounded generic project type and an optional pair of declared adapter ID and exact adapter version. Format version 1 identifies the shape; an optimistic revision starts at 1 and rejects stale `expected_revision` values with `PROJECT_CONFIG_CONFLICT`. Unconfigured projects return revision 0 and `configured: false`.

The adapter binding is declarative metadata. It is not proof that an adapter is installed, compatible, permitted, or has parsed a project file. The configuration does not contain adapter-specific settings, credentials, machine paths, or tool-specific rules; later integration contracts may extend it by a new format version. A caller's producer metadata never grants authority over dependency edges. The existing `project.dependencies.replace` transaction still requires ready index state, current generation, matching source digest, and indexed project-relative targets.

## Verification

- A schema-6-to-7 migration fixture preserves an existing stale index generation and its content-verification requirement while leaving configuration absent.
- Core service tests create two projects, write one declared adapter binding, reject a stale revision and a one-sided adapter pair, deny a cross-project read, then reopen storage and confirm only the intended project's configuration persisted.
- A live daemon/client test writes configuration, hard-kills the daemon, reopens it, reads the same revision/version, and rejects a stale update. Parallel fixtures now use unique daemon instance names.
- Registry self-validation, the full workspace suite (79 active tests), the focused version-string test, and the release workspace build pass. The ignored 15,000-file benchmark was not rerun for this storage/contract change.

## Remaining gate

No parser is installed or dispatched by this slice. The daemon does not yet feed adapter observations into dependency-edge replacement. The next implementation must bind an installed parser to this declared version, validate bounded project-relative observations against the current indexed source digest, then replace edges through Core only after an authorized and sandboxed invocation. Project source files stay authoritative and parser output stays untrusted.
