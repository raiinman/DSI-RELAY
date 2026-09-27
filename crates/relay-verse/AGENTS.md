# Purpose

Own bounded normalization of structured Verse telemetry and deterministic gameplay assertions.

# Ownership

- Governs `crates/relay-verse/`.
- Consumes caller-authorized local capture text; it does not access UEFN, Fortnite, project files, network services, or RELAY storage.

# Local Contracts

- Accept only `RELAY_EVENT_V1 ` JSONL records with the declared session ID, version, and bounded token fields. Reject malformed records without echoing raw log text.
- Preserve provenance, rejected counts, truncation, sequence gaps, and session boundaries. An incomplete capture cannot prove an event was absent.
- Assertion outcomes describe local captured evidence. A synthetic or imported capture never proves a live UEFN workflow passed; even caller-declared UEFN logs require independent adapter verification.
- Project-specific event names and assertions are data, not hard-coded game rules.

# Work Guidance

- Keep the parser side-effect free and reject unbounded payloads and private paths.
- The calling command must authorize the project and obtain source text through the appropriate adapter before invoking this crate.

# Verification

- Run a crate build check after parser or assertion changes.

# Child DOX Index

- No child DOX files currently exist.
