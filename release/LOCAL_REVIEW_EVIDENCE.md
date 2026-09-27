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

## Fresh hosted Windows and current local bundle

The [hosted review run 36287058192](https://github.com/raiinman/DSI-RELAY/actions/runs/36287058192) passed from clean revision `20a40228e576a60cb6b0c2ae5b3bfacd72c430e4`. It verified the assembled ZIP and fresh extraction at 98 hashed files, then completed one two-project, 100-file-per-project sample with all seven correctness flags true. The workflow checked report privacy and found no residual daemon or owned benchmark directory before uploading only `hosted-runner-smoke.json`. The downloaded report SHA-256 is `83d3186068c23f6f5d34c923ef0f75557bf2c29986cf992ac06c6e3173caf734`. It records a hosted 2-core/4-thread, 16 GiB VM; this is one synthetic sample, not a supported hardware-tier result or an ordinary desktop-account test.

`0.1.0-preview.12` was assembled locally from that same clean revision after the hosted run. Its archive SHA-256 is `caea21008c05b7f78026279f777d5d8101065f3c9aca707ce7849bcdd5fd9971`; the bundle verifier checked 98 hashed files. The contained runner passed one 100-file-per-project sample under Windows PowerShell 5.1, with no residual daemon or owned temporary directory. The local report identifier scan passed. The archive remains local and review-only. Its manifest still lists `CLEAN_ACCOUNT_SMOKE_PENDING`, `LEGAL_RELEASE_APPROVAL_PENDING`, and `PUBLISHER_TRUST_AND_SECURITY_REVIEW`.

## Host-token hardening and temporary-account CI

The daemon now creates `host.json` with an explicit protected current-user ACL and verifies the kernel owner and DACL before writing its per-start connection token. The full locked workspace test suite passed locally after this change. The new hosted CI path builds from committed source, then runs the contained synthetic benchmark under a fresh temporary non-administrator account using secondary logon. It screens both JSON reports and removes the account and owned staging data before finishing. [Hosted run 36288650624](https://github.com/raiinman/DSI-RELAY/actions/runs/36288650624), attempts 1 and 2, passed the ordinary and temporary-account checks from revision `0e36ea165388fca154e230a95dc3a422f89c5726`. Both account reports had one completed 100-file sample and all seven correctness flags true. This is separate-identity/profile evidence, not a full interactive desktop sign-in or a minimum hardware-tier result. A prior broad environment-filter attempt failed its benchmark result check and is not counted as passing evidence.

`0.1.0-preview.17` was assembled locally from that same clean revision. Its archive SHA-256 is `fe891e5c98b53869aa64da036be1e9dc47843d2b28dbba5310af492f6b816a36`; the assembled folder and fresh extraction each verified 98 hashed files. The contained one-sample, two-project 100-file run passed and left no residual daemon or owned temporary directory. The report privacy scan passed. The bundle is unsigned and remains local review only, with `CLEAN_ACCOUNT_SMOKE_PENDING`, `LEGAL_RELEASE_APPROVAL_PENDING`, and `PUBLISHER_TRUST_AND_SECURITY_REVIEW` still in its manifest.

## Draft release and repository visibility

GitHub currently reports `raiinman/DSI-RELAY` as a public repository. Its `v0.1.0-preview.17` release remains an unpublished draft, marked prerelease, with the ZIP and checksum attached. The uploaded ZIP digest matches `fe891e5c98b53869aa64da036be1e9dc47843d2b28dbba5310af492f6b816a36`. The draft does not authorize or expose a public binary download. Phase 3 PR #2 remains a draft against `main`; the later Phase 4 and Phase 5 work is in separate draft branches and is not included in this pinned ZIP.

The hosted [review run 36289036139](https://github.com/raiinman/DSI-RELAY/actions/runs/36289036139) passed from the latest Phase 3 branch documentation revision, after the candidate ZIP's pinned source revision. It did not rebuild or replace the attached ZIP. The public-source preflight found one Git commit author identity, using a GitHub noreply address; targeted current-tree and diff-history checks found no local account path, private-key header, GitHub token prefix, or AWS access-key pattern. These limited checks are not a comprehensive secret audit.
