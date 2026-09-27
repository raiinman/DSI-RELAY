# Phase 3 automatic continuity recovery evidence

Status: PASS for the synthetic Windows recovery fixture, 2026-09-26. Phase 3 remains active.

## Contract

Schema 6 adds `content_verification_required` to each project index state. Migration marks existing baselines stale and requires a full content pass because a daemon upgrade includes an interval without trusted notification continuity. Watcher downtime, failed subscriptions, dropped events, and ambiguous filesystem events set the same durable flag without advancing the index generation. Hint-only updates preserve it. A metadata-only `project.index.reconcile` returns `INDEX_CONTENT_VERIFICATION_REQUIRED` while it is set; only a verified reconciliation or full baseline rebuild can restore `ready`.

The daemon schedules at most one stale project per poll after a two-second event quiet period and 30 seconds of signed-in-session input inactivity. It also waits for no active RELAY command or pending hint batch. The recovery uses full content verification when the durable flag is set; ordinary provisional hints use metadata reconciliation. Enumeration and 64 KiB hash reads check the activity guard. If input or foreground command activity resumes, the plan is discarded before the storage commit. Metadata-only attempts also stop after 15 seconds; required full-content verification may continue while the session remains idle and no watcher callback changes the event epoch. Failed or deferred attempts back off for 30 seconds. A project whose root has no active watch subscription is skipped. These are first implementation limits, not supported hardware-tier budgets. The later policy change and focused test are recorded in `PHASE3_WATCHER_LOAD_EVIDENCE.md`.

The Windows idle signal uses [GetLastInputInfo](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getlastinputinfo) and [GetTickCount](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-gettickcount). The signal is specific to the daemon's Windows session; it does not claim knowledge of GPU, disk, thermal, or other sessions' activity. Ambiguous tick data defers work.

## Verification

- The schema-4-to-6 and explicit schema-5-to-6 fixtures preserve project generations and prior records while conservatively requiring verification. A storage-level metadata commit is rejected after a hard reopen, and a verified commit clears the flag.
- A Core indexing test interrupts a guarded scan and confirms no partial plan is returned.
- A live daemon test imports a project, kills the daemon, rewrites a file with equal-length bytes and its original timestamp, then restarts. The immediate capability report is stale and requires verification. Metadata-only reconciliation fails. The idle worker restores ready state and the project change delta identifies the edited file, without a manual recovery command.
- The Windows watcher suite passes three active tests; the 15,000-file scale fixture remains ignored except when invoked manually. The full workspace suite passes 76 active tests; the release workspace build passes.

## Limits and remaining work

The live test accelerates the idle threshold in a debug daemon; release builds retain 30 seconds. The fixture proves one small project and one restart, not event-storm endurance, foreground frame-time impact, or recovery completion on minimum/recommended hardware. Required full verification can continue beyond 15 seconds during an idle, quiet period; repeated interruptions can still leave a project stale until explicit reconciliation or a future resumable design. The scheduler uses input inactivity as a foreground proxy; it does not yet measure creator-app pressure. Supported-tier budgets and foreground editor testing remain Phase 3 closure gates. Later slices added parser dispatch and configuration/version metadata; see `PHASE3_CLOSURE_REVIEW.md` for current status.
