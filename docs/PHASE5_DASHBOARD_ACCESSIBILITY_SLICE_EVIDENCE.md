# Phase 5 dashboard accessibility slice

Status: implemented for the existing local, read-only health and project view. Phase 5 remains open.

The page now has a keyboard skip link and a single atomic status announcement for refresh progress, success, and failure. Refresh remains focusable while a request is in flight and ignores duplicate activation. Failed checks appear by plain-language component name, with exact command data still under Advanced details. The doctor's next action is visible when available. Project IDs remain in Advanced details; names are inserted as text.

If a refresh fails, the view clears previously shown health, project, next-action, and raw command data. An expired dashboard token points the user to a fresh link. Browser code still calls only `system.status`, `system.doctor`, and `project.list` through the daemon's read-only HTTP command path. It adds no browser storage, external asset, or new command behavior.

Verification: `node --test crates/relayd/dashboard/app.test.mjs` passed four tests covering the command set and token header, changed status text, safe project-name insertion, visible check names, stale-data clearing, and expired-token guidance. `cargo test -p relayd --bin relayd dashboard::tests --locked` passed three tests; `cargo test -p relayd --test phase5_dashboard --locked` passed one. `cargo test --workspace --locked` and `cargo build --release -p relayd --locked` passed. These automated checks do not replace a manual screen-reader and keyboard session across supported browsers, zoom levels, and Windows contrast settings. Project switching, onboarding, jobs, approvals, and the wider Phase 5 accessibility review remain open.

A disposable local daemon started for a live browser pass, then was stopped. The available computer-use inventory exposed no browser, and the in-app browser was unavailable. No live keyboard, zoom, visual, or screen-reader result was collected.
