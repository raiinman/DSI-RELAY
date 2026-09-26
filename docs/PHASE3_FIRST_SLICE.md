# Phase 3 first slice

Status: Active.

Goal: promote one generic multi-project discovery/indexing vertical into the production Rust workspace without adding tool-specific behavior.

## Scope

The first slice must prove together:

- two synthetic local projects can be added/imported through the canonical command path
- project roots are normalized locally and stored behind opaque project IDs
- project A and project B baseline/index state never cross project scope
- one durable baseline file inventory survives daemon restart
- one compact capability report is available per project
- one watcher hint can trigger changed-only processing
- authoritative reconciliation detects a change that produced no watcher hint
- unchanged files are not reparsed for an ordinary one-file update
- deleted and renamed files are represented explicitly in the derived state
- index state is rebuildable from authoritative project files
## Storage and contracts

The slice may advance operational SQLite beyond schema 3 only if the migration is focused and backward-tested.

Any new durable index tables must:

- be keyed by project ID
- store project-relative path identity instead of exposing absolute paths through public results
- carry schema/version metadata sufficient for rebuild/migration
- distinguish baseline generation/revision from live watcher hints
- remain derived state that can be discarded and rebuilt without modifying project files

New commands/capabilities must be added to the shared command registry rather than hard-coded in daemon/CLI surfaces.

## Acceptance

1. two projects coexist with no state leakage;
2. project add/import is deterministic and non-interactive in machine mode;
3. a baseline inventory survives graceful and hard restart;
4. project capability report works through the canonical CLI/daemon command path;
5. changing one file updates only that file's indexed record under a healthy baseline;
6. a deliberately missed watcher event is recovered by reconciliation;
7. delete/rename handling does not require a normal full rescan;
8. malformed/unreadable paths fail with structured errors rather than partial hidden state;
9. resource measurements cover two inactive projects plus one changed-file update;
10. no UEFN/Fortnite/editor-specific behavior enters Core.
## Synthetic fixture

Use generic temporary directory projects only.

Recommended fixture shape:

- Project A: 100-500 small text/source-like files across nested directories
- Project B: separate tree with overlapping filenames to prove isolation
- include ignored/noise files only if an explicit generic ignore rule is part of the slice
- change one file, rename one file, delete one file, and create one file
- deliberately suppress/skip one watcher hint, then reconcile

The fixture may use generic extensions such as `.txt`, `.json`, and `.src`; it must not encode Fortnite/UEFN semantics.

## Measurement

Record at minimum:

- initial baseline scan time
- files discovered/indexed
- changed-only update latency
- files reparsed per one-file change
- reconciliation latency
- delete/rename recovery
- baseline/index database growth
- daemon idle RSS/CPU with zero, one, and two registered projects
- restart recovery/readiness time
- full rebuild time as a recovery control, not the ordinary update path
## Closure evidence

Create `PHASE3_FIRST_SLICE_EVIDENCE.md` only after the implementation passes its tests and resource checks.

The evidence must state exact schema migration consequences, command IDs/versions, failure behavior, restart behavior, and whether any assumption from D-149 or Phase 2 had to change.
