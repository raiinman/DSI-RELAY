# Portable Phase 3 synthetic benchmark draft

Status: local review draft, 2026-09-26. No standalone binary bundle has been published.

`crates/relayd/tests/phase3_portable_benchmark.ps1` can run with two release binaries and Windows PowerShell, without a Rust checkout or Cargo on the test machine. The current local review archive contains:

| File | Purpose |
| --- | --- |
| `relayd.exe` | Per-user daemon under test. |
| `relay.exe` | Canonical machine-mode command client. |
| `run-phase3-benchmark.ps1` | A copy of the reviewed script, beside the two binaries. |
| `README.txt` | Plain-language run steps, expected duration and disk use, data fields, and manual report-sharing choice. |
| `SHA256SUMS.txt` and `bundle-manifest.json` | Hashes and provenance details for the complete bundle, including third-party notices. |

The draft script uses a unique temporary directory it creates for each run, sets a private daemon state and instance for its child process, generates two generic text projects, exercises the live import/index/watch/reconcile command path, and removes only its verified owned directory after the daemon exits. It rejects a reparse-point/junction substitution at the root or in descendants before recursive cleanup and requires the output report to stay outside that directory. CLI calls have a deadline and a stalled child is killed before daemon cleanup. It never reads a user's project and contains no remote network call or upload; the daemon opens a loopback-only dashboard listener during each sample.

The output JSON is local and opt-in. It records each raw run and explicit correctness outcomes, plus benchmark/schema version, daemon/storage version, binary hashes, file counts, timing and idle/storage samples, CPU model/core counts, RAM bytes, OS version, storage class, and a coarse power-plan class. It omits usernames, computer names, volume labels, project paths, source contents, the IPC token, raw command output, and a persistent device identifier. The script refuses an existing report path, writes the initial report through a same-directory move, and atomically replaces later progress records. A handled failure records `failed`, a fixed failure code, and the stage without raw daemon/CLI text; a hard interruption leaves the most recent completed report write. The user should inspect the JSON before choosing to share it.

Example for a reviewer with existing local release binaries:

```powershell
.\crates\relayd\tests\phase3_portable_benchmark.ps1 -BinaryDirectory .\target\release -FilesPerProject 1000 -Runs 3 -OutputPath .\relay-benchmark.json
```

A two-repeat 100-file-per-project smoke run against the existing local release binaries passed. A later one-repeat smoke verified the storage class field, and a missing-binary check verified that failure status and stage remained in a local JSON report. The hardened runner then passed one 100-file sample under Windows PowerShell 5.1. An intentional 1 ms CLI deadline yielded fixed `BENCHMARK_CLI_TIMEOUT`, no remaining daemon or fixture root; a rerun against an existing report failed without changing its SHA-256. These are functional checks, not budget results: a 100-file fixture is smaller than the 15,000-file tier fixture. A separate clean-account run remains open.

Before any public archive, build the exact release binaries from a reviewed revision, record provenance and hashes, verify the script with those binaries on the supported Windows versions, and review the archive's licensing and signing posture. The current test-only `relay-adapter-fixture.exe` must not be included in public packaging. This first bundle measures generic indexing and watcher cost only; a public parser benchmark would require a separately reviewed, distributable sample parser or real supported adapter. Minimum/recommended hardware budgets and creator-app interference still require external machines or sessions; this draft does not claim them.
