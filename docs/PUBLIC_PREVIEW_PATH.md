# Path from local Phase 3 evidence to a public RELAY preview

Status: planning guide, 2026-09-26. No public artifact is approved or published by this document.

## Separate the two release claims

**Measurement preview:** a small, locally run package creates synthetic projects, exercises release-built preview daemon and CLI binaries, and writes a report for a tester to inspect and share manually. It can collect evidence from slower Windows PCs before the full product is ready. It must not read a tester's existing projects or upload anything automatically. Results are exploratory until repeated across declared hardware tiers.

**Public RELAY beta:** a new user installs the product, adds a supported project, sees useful results, recovers from common failures, and can update or uninstall without losing project data. The current Phase 3 checkout does not yet meet this product promise. The roadmap's later context, adapter, dashboard, automation, and public-hardening phases remain distinct work.

Neither claim requires a central hardware-tracking service. A versioned local JSON report plus an opt-in submission path is sufficient to begin collecting tier evidence. A service becomes useful only if the volume of reports makes manual review impractical, and would add privacy, security, hosting, and support obligations.

## Minimum measurement-preview artifact

1. Build the Rust binaries once in release mode and package only RELAY-owned preview binaries, a bounded synthetic fixture runner, a report schema, instructions, and required third-party notices. Do not include the synthetic sandbox fixture worker as a public adapter.
2. The runner creates fresh temporary projects, records baseline, watcher, one-file update, full verification, storage growth, idle CPU/RSS, and correctness outcomes, then removes only its own temporary data. A failure must still preserve a safe, inspectable report and clean up owned test processes.
3. The report records RELAY build and report-schema versions, Windows build, CPU model/core count, installed RAM, storage class, power mode when available, fixture shape, raw timing samples, work counts, failure codes, and whether the run was interrupted. It excludes account names, machine names, project paths/content, credentials, and raw logs. Testers preview the file and choose whether to submit it.
4. Run the package on a clean Windows account before sharing it. Verify no network access is needed, no unrelated project files are opened, no background daemon remains after the run, and removal leaves user data untouched.
5. Publish precise limitations: synthetic indexing evidence only, no real editor integration, no supported-tier performance promise, no automatic uploads, and no production-ready installer claim.

The current `crates/relayd/tests/phase3_scale_report.ps1` is an internal Rust-checkout runner. The draft `crates/relayd/tests/phase3_portable_benchmark.ps1` uses release-built preview binaries without Cargo. Two-repeat and one-repeat local synthetic smoke runs passed, and a missing-binary preflight failure wrote a bounded local report. The report fields and cleanup still need clean-account review; dependency notices and distribution trust also remain open. See `PHASE3_PORTABLE_BENCHMARK_DRAFT.md`.

## Before any public binary distribution

- Choose the RELAY license, inventory dependency licenses, and include required notices. Review branding and any included third-party components. The public-release legal checklist is in `LEGAL_LICENSING_AND_DISTRIBUTION.md`.
- Decide the publisher-signing and trust bootstrap for a direct Windows package. The Phase 1 side-by-side updater is prototype evidence, not a shipping updater; do not distribute its synthetic test certificate. For a preview without unattended updates, publish a verified versioned bundle and an explicit manual replacement path with schema compatibility checks.
- Complete a focused threat/privacy review of the packaged surface, including named-pipe cross-user denial, path traversal, archive extraction, adapter execution, diagnostic redaction, and clean removal. Publish a privacy notice that matches the actual no-upload behavior.
- Provide a support channel, known limitations, reproducible bug-report fields, and a clear way to report security issues. Test install/run/update or replacement/uninstall from a clean user profile.

These are release controls, not Phase 3 indexing work. Passing Phase 3 unit and synthetic daemon tests alone does not make a public binary safe or useful.

## From volunteer reports to supported tiers

Use the same versioned fixture on each candidate machine, keep raw samples, and reject reports with failed correctness checks. Group results by actual machine class and storage type; do not infer a minimum-tier pass from CPU throttling on a fast workstation. Select numerical limits only after enough clean runs exist for each tier, then test foreground interference with a real creator application.

The current [Epic UEFN requirements](https://dev.epicgames.com/documentation/fortnite/install-and-launch-fortnite-creative-and-unreal-editor-for-fortnite) describe a 16 GiB minimum and 32 GiB recommended memory class, with a four-core 2.5 GHz CPU for both. Those published classes can help recruit testers, but a machine's actual storage, GPU, power policy, and concurrent editor load must be recorded separately. The high-end one-host Phase 3 samples in `PHASE3_RESOURCE_BUDGET_METHOD.md` are not substitutes for those runs.

Once supported-tier, foreground, and recovery evidence pass, recheck `PHASE3_CLOSURE_REVIEW.md`. A public RELAY beta additionally needs the roadmap's onboarding, real integration, dashboard, distribution, legal, privacy, and support gates.
