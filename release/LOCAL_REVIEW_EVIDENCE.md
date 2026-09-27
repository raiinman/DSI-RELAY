# Measurement-preview local assembly evidence

Status: local review only, 2026-09-26. No archive was published or signed.

The `0.1.0-preview.7` review bundle was assembled after the dashboard and adapter source edits with a separate remapped Rust release target. Its archive SHA-256 was `70dddb00f30dfeffcdd0758bc949d1d973dc97eb88c760b9890d7ec885492da1` in two independent output directories with identical inputs. The bundle verifier checked 96 hashed files. The manifest contained 42 third-party crates, 95 payload files, the exact lockfile hash, and RELAY's MIT license and RAiiNMAN attribution.

The contained runner completed one 100-file-per-project sample with `status=passed`, wrote a local report, and left no `relayd.exe` process or benchmark temporary root. The report and nonbinary payload text contained no local account name, Windows user-profile path, or token string; a raw-byte scan of the package and report found no current machine name. The packager rejected ordinary unremapped `target/release` binaries because they embedded local Cargo source paths. The remapped binaries passed the package's personal-path scan. A prior local tamper check changed `README.txt`; the bundled verifier rejected the changed file and passed again when its original bytes were restored.

The manifest marks `source_tree_clean=false` because this bundle was built from active shared-worktree edits. It is not a provenance-attested or public-ready binary. Regenerate after a reviewed commit and verify the new archive before any release decision.

The `.7` manifest flagged bundled SQLite provenance, human selection/review of third-party notice obligations, clean-account smoke, and publisher trust/security review. The 100-file sample is functional smoke, not minimum/recommended hardware or real creator-app evidence. No lower-spec Windows machine or creator session was available for this review.

## Notice and SQLite review follow-up

The `0.1.0-preview.9` local bundle included a pinned license-file fingerprint for the same 42-crate Windows graph and a technical SQLite 3.53.2 provenance record. Two earlier `0.1.0-preview.8` archives matched SHA-256 `f843c37357b59f2d45444953f612b29bb0e13e49074090c994a780e8b7afcfe5`; the refreshed `.9` bundle passed the verifier on 98 hashed files. The packager rejected an inherited `CARGO_ENCODED_RUSTFLAGS` setting with a nonzero exit, preventing caller flags from silently changing the normal remapped build. The full findings are in `THIRD_PARTY_NOTICE_REVIEW.md`.

The `.9` manifest clears only the two technical checks for notice selection and bundled SQLite source provenance. It keeps `CLEAN_ACCOUNT_SMOKE_PENDING`, `LEGAL_RELEASE_APPROVAL_PENDING`, and `PUBLISHER_TRUST_AND_SECURITY_REVIEW`. The `.9` worktree was dirty; the committed-source artifact must be regenerated and checked separately.

## Committed-source review bundle

`0.1.0-preview.10` was built by the packager from clean revision `a255f9d51b601f23555eb056473bb2da118fe5ca`. Its manifest reports `source_tree_clean=true`, `binary_build_mode=built_by_packager`, 42 third-party crates, verified notice selection and SQLite provenance, and the same three remaining release checks. The archive SHA-256 is `3e3620d4473cf6fb286993c00c9de690e0bd7307449fc142fc90224e5b926589`; the verifier checked all 98 hashed files.

The contained runner passed one local two-project smoke sample with 100 synthetic files per project. The generated JSON report had `status=passed`, contained no current account name, computer name, user-profile path, or Windows user-root string, and left no `relayd.exe` process. This is one-host functional evidence, not a clean-account test, supported hardware-tier result, or real creator-app test. The archive remains local and review-only. The manifest also states that binary-to-revision correspondence is not independently attested.

## Hardened runner review bundle

`0.1.0-preview.11` was built from clean revision `70f89559ab5604b74c74b26453c35d0685d49fb3` after the report and CLI-deadline hardening. The archive SHA-256 is `f3ff7e4a287b9b8cd8c6c4938d80fd07bd3024d3098b9cf4a825856fff3927c6`; the bundle verifier checked 98 hashed files. The manifest reports a clean tree, packager-built binaries, and the three remaining release checks. The included README discloses the loopback dashboard listener and links to the revision-specific privacy and support policies.

Under Windows PowerShell 5.1 on the existing workstation, the contained runner passed one 100-file-per-project sample. An intentional 1 ms CLI deadline produced `BENCHMARK_CLI_TIMEOUT` and a local `failed` report. Neither run left a daemon or owned benchmark directory. The successful report contained no current account name, computer name, or user-profile path. An existing-report test against the same reviewed runner failed before daemon startup and preserved the report SHA-256. A separate standard-account test and a live descendant-junction injection test remain open; this evidence does not authorize public distribution.
