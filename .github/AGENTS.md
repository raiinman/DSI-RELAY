# Purpose

Own repository automation for review and validation.

# Ownership

- Governs `.github/` and descendants.
- The root AGENTS.md governs project-wide behavior and release claims.

# Local Contracts

- Preview workflows run only on the named development branch and upload a sanitized smoke report. A public repository's workflow artifacts are readable by others, so the workflow must not upload the binary archive before public distribution is approved.
- Keep build jobs at minimum required permissions. Binary attestation can be added with a separate narrow-permission job when binary distribution is approved.
- Validate the archive, contained benchmark, report privacy, and process/temp cleanup before uploading a review artifact.
- Hosted-runner smoke is fresh-environment evidence, not a separate standard desktop account or supported hardware-tier proof.

# Verification

- Parse workflow YAML and PowerShell scripts before enabling CI.
- Run the workflow on the target branch; inspect the actual GitHub run and artifact before citing it as passing evidence.

# Child DOX Index

- No child DOX files currently exist. This file owns workflows and their small helper scripts.
