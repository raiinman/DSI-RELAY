# Purpose

Own bounded, privacy-safe support bundle assembly for RELAY.

# Ownership

- Governs `crates/relay-support/`.
- Accepts typed health, aggregate, version, and integrated-run summaries. It never reads raw logs or files and does not write the bundle.

# Local Contracts

- Emit a versioned JSON bundle with diagnostic health/counts, component versions, integrated outcome counts, and stable reason-code counts.
- Reject paths, command arguments, secrets, raw log text, malformed codes, oversized input, and oversized output. Do not include arbitrary user text or raw event history.
- If the integrated run has not happened, report that as absent; do not fabricate a pass or substitute synthetic evidence.
- A supplied integrated report is parsed under a byte limit and reduced to checked outcome counts, reason-code counts, and resource totals; it is a summary of that file, not independent attestation of its observations.

# Work Guidance

- Keep assembly deterministic and side-effect free. The caller maps Core diagnostics and final report into the narrow input types.
- Use stable registered codes for component IDs and reason codes; no personal identifiers.

# Verification

- Run focused crate tests for privacy, report consistency, and output bounds.

# Child DOX Index

- No child DOX files currently exist.
