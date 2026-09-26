# Phase 3 automatic continuity recovery evidence

Status: PASS for the synthetic Windows recovery fixture, 2026-09-26. Phase 3 remains active.

## Contract

Schema 6 adds `content_verification_required` to each project index state. Migration marks existing baselines stale and requires a full content pass because a daemon upgrade includes an interval without trusted notification continuity. Watcher downtime, failed subscriptions, dropped events, and ambiguous filesystem events set the same durable flag without advancing the index generation. Hint-only updates preserve it. A metadata-only `project.index.reconcile` returns `INDEX_CONTENT_VERIFICATION_REQUIRED` while it is set; only a verified reconciliation or full baseline rebuild can restore `ready`.

The daemon schedules at most one stale project per poll after a two-second event quiet period and 30 seconds of signed-in-session input inactivity. It also waits for no active RELAY command or pending hint batch. The recovery uses full content verification when the durable flag is set; ordinary provisional hints use metadata reconciliation. Enumeration and 64 KiB hash reads check the activity guard. If input or foreground command activity resumes, or a 15-second attempt limit expires, the plan is discarded before the storage commit. Failed or deferred attempts back off for 30 seconds. A project whose root has no active watch subscription is skipped. These are first implementation limits, not supported hardware-tier budgets.

The Windows idle signal uses [GetLastInputInfo](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getlastinputinfo) and [GetTickCount](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-gettickcount). The signal is specific to the daemon's Windows session; it does not claim knowledge of GPU, disk, thermal, or other sessions' activity. Ambiguous tick data defers work.

## Verification

- The schema-4-to-6 and explicit schema-5-to-6 fixtures preserve project generations and prior records while conservatively requiring verification. A storage-level metadata commit is rejected after a hard reopen, and a verified commit clears the flag.
- A Core indexing test interrupts a guarded scan and confirms no partial plan is returned.
- A live daemon test imports a project, kills the daemon, rewrites a file with equal-length bytes and its original timestamp, then restarts. The immediate capability report is stale and requires verification. Metadata-only reconciliation fails. The idle worker restores ready state and the project change delta identifies the edited file, without a manual recovery command.
- The Windows watcher suite passes three active tests; the 15,000-file scale fixture remains ignored except when invoked manually. The full workspace suite passes 76 active tests; the release workspace build passes.

## Limits and remaining work

The live test accelerates the idle threshold in a debug daemon; release builds retain 30 seconds. The fixture proves one small project and one restart, not event-storm endurance, foreground frame-time impact, or recovery completion on minimum/recommended hardware. Projects that cannot finish within a 15-second attempt remain honestly stale and need an explicit reconciliation or a later chunked recovery design. The scheduler uses input inactivity as a foreground proxy; it does not yet measure creator-app pressure. Automatic dependency extraction, configuration/version metadata, supported-tier budgets, and long-run watcher soak remain Phase 3 closure gates.
