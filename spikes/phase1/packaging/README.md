# Spike 13 Packaging/Update Candidates

Spike 13 compares two Windows distribution/update shapes using the same synthetic Rust core + adapter payload.

## Selected default — signed side-by-side

D-158 selects the per-user side-by-side model for the default personal/direct-download path.

- ZIP payload + separate CMS signature over the bundle SHA-256 digest
- release manifest inventories version/channel/source/storage compatibility and component hashes/provenance
- verify signature/publisher/digest/component hashes before staging
- stage into a version directory without changing the active version
- atomically activate through a tiny fsync'd `current.json` pointer
- refuse rollback when durable SQLite schema is newer than the candidate binary supports
- keep durable RELAY/project data outside the binary/version root
- uninstall binaries without deleting durable data

The Node updater here is evidence code only. A shipping updater must implement this model in the selected shipping stack and define a production signer/key trust bootstrap without installing a test root.

## Optional channel — MSIX + App Installer

MSIX remains useful for Microsoft Store or managed/direct distribution when a certificate Windows already trusts is available. Windows can then own package registration, repair/uninstall and App Installer update scheduling.

The fixture built valid signed MSIX packages, verified component inventories/tamper detection, and generated upgrade-only App Installer metadata. Non-admin registration of the self-signed fixture was blocked by Windows certificate trust, so Spike 13 does not claim the MSIX lifecycle was physically proven on this machine.
