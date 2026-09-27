# Local measurement-preview bundle

`Build-MeasurementPreview.ps1` assembles a **review-only** Windows archive for the Phase 3 synthetic benchmark. It does not publish or sign the archive. Its generated `README.txt` explains the limited claim and local JSON privacy behavior to a tester.

The benchmark daemon opens a local named pipe and a loopback-only dashboard listener while each sample runs. The runner does not make a remote network call or upload its report; this is disclosed in the generated README and root `PRIVACY.md`.

Run from the repository root after reviewing the source revision:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\release\Build-MeasurementPreview.ps1 -Version 0.1.0-preview.1
```

The default builds `relay.exe` and `relayd.exe` in release mode with the lockfile in a separate `target/measurement-preview/release/` tree. It rejects inherited `RUSTFLAGS` and `CARGO_ENCODED_RUSTFLAGS`, applies only its reviewed path-remapping flags, and rejects binaries containing the current account name or a Windows user-profile path. Ordinary `target/release/` binaries can contain personal Cargo cache paths and fail this check. `-SkipBuild -BinaryDirectory <path>` packages already built, screened binaries for a local review, but the manifest then cannot attest their source. The output goes to ignored `release/out/` unless `-OutputDirectory` is supplied. An existing versioned folder, zip, or zip checksum is never overwritten; use a new version for changed payloads. The script validates and removes only its own GUID-named staging directory.

The versioned folder and zip include the two binaries, the reviewed portable benchmark runner, a local `verify-bundle.ps1` hash checker, plain-language instructions and privacy/limitations, a clean-account smoke checklist, `bundle-manifest.json`, `SHA256SUMS.txt`, a machine-readable inventory of the exact Windows CLI/daemon resolved dependency graph, and available license/copyright files from cached crate sources. A sibling `.zip.sha256` records the archive hash. The manifest includes the repository revision and clean/dirty state, lockfile hash, per-file sizes and hashes, and explicit unresolved review checks. It does not establish that a supplied binary was built from that revision.

The script requires the offline Cargo registry cache used by the lockfile. It copies all matching license/notice files at each crate root and any `license_file` declared in metadata. For the reviewed lockfile, it records the MIT choice where a crate offers it, while retaining all available original license texts; it also retains Unicode-3.0 and CC0-1.0 texts where applicable. A pinned fingerprint detects changes to the 42-crate graph, choices, or copied license-file bytes and reopens the notice review. A separate pin checks the SQLite 3.53.2 amalgamation and bundled feature choice against the upstream digest review. These are technical checks, not legal distribution approval. A missing RELAY `LICENSE` or `NOTICE-RELAY.txt` is recorded in the manifest rather than replaced with an invented owner name.

Before considering distribution, complete the unresolved manifest checks, run the contained benchmark from a clean standard Windows account, inspect the report, verify no residual daemon or temporary directory, review the package security/privacy posture, and arrange publisher trust and support. The generated checklist is a plan, not a claim that those checks passed. Phase 3 hardware-tier and real creator-app evidence remain separate.

To verify a local folder, compare every line in `SHA256SUMS.txt` against the corresponding file's SHA-256 and compare the sibling `.zip.sha256` with the archive. The archive is written in sorted file order with fixed entry timestamps for repeatable bytes when the inputs and packaging runtime stay identical.

The latest local check and remaining distribution gates are recorded in `LOCAL_REVIEW_EVIDENCE.md`. The exact source and license-text review is in `THIRD_PARTY_NOTICE_REVIEW.md`.
