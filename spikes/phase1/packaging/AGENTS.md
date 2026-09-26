# Purpose

Own Phase 1 synthetic Windows packaging/update candidates and fixtures used to select RELAY's install/update/rollback model.

# Ownership

- This file governs `spikes/phase1/packaging/`.
- `spikes/phase1/AGENTS.md` remains authoritative for shared Phase 1 evidence rules.
- Durable product packaging architecture belongs in `docs/`; this folder supplies candidate evidence only.

# Local Contracts

- Keep package/install fixtures synthetic and generic; never package private project content, credentials, user paths, or generated signing secrets.
- The default personal install candidate must remain per-user and must not require a Session 0 service.
- Signing certificates/keys used for local tests are ephemeral current-user fixtures and must be removed after the test; never commit private keys or certificates.
- Durable RELAY/project data stays outside versioned package payloads so uninstall/binary rollback cannot silently delete or reinterpret user state.
- Update activation happens only after package/component integrity and version metadata validate.
- Binary rollback and data-schema recovery are separate decisions; an older binary must fail closed on unsupported newer storage.
- Compare MSIX/App Installer against a minimal side-by-side/atomic-activation updater using the same synthetic payload and failure cases.
- Generated MSIX/AppInstaller/certificate/binary outputs stay outside Git; only sanitized benchmark evidence is committed.

# Verification

- Run the neutral Spike 13 benchmark on Windows.
- Verify test package/certificate cleanup and no remaining synthetic installed package.
- Verify failed/tampered update leaves the prior active version intact.
- Verify external durable data survives install/update/downgrade/uninstall.
- Run `git diff --check` and scan for generated packages/keys/certs before staging.

# Child DOX Index

- No child DOX files currently exist under this folder.
