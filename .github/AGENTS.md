# Purpose

Own repository automation for review and validation, and public issue intake templates.

# Ownership

- Governs `.github/` and descendants.
- The root AGENTS.md governs project-wide behavior and release claims.

# Local Contracts

- Preview workflows run only on the named development branch and upload a sanitized smoke report, or public dependency metadata after failure. A public repository's workflow artifacts are readable by others, so the workflow must not upload the binary archive before public distribution is approved.
- The active development branch has a build-only check that uploads no binary or project data. It is construction feedback, not the final integrated workflow run.
- Keep build jobs at minimum required permissions. Binary attestation can be added with a separate narrow-permission job when binary distribution is approved.
- Validate the archive, contained benchmark, report privacy, and process/temp cleanup before uploading a review artifact.
- The optional hosted standard-account smoke uses a fresh local non-administrator identity and profile with only the extracted package, uploads screened JSON, and removes its account and owned staging directory. It is secondary-logon evidence, not an interactive desktop sign-in or supported hardware-tier proof.
- Benchmark issue intake is manual and opt-in. Templates must make public visibility and local report inspection clear; never request private project content, credentials, raw logs, or automatic uploads.

# Verification

- Parse workflow YAML and PowerShell scripts before enabling CI.
- Run the workflow on the target branch; inspect the actual GitHub run and both JSON artifacts before citing either check as passing evidence.

# Child DOX Index

- No child DOX files currently exist. This file owns workflows and their small helper scripts.
