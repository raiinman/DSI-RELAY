# Purpose

Own the first UEFN-specific adapter boundary.

# Ownership

- Governs `crates/relay-uefn/`.
- Receives project-relative metadata from the generic RELAY project index; never owns Core state or transport.

# Local Contracts

- Static inspection is read-only and deterministic. It does not open UEFN files, invoke UEFN, or infer live editor/runtime capability from filenames.
- A top-level `.uefnproject` filename is a project marker, not proof that the manifest parses or that UEFN is installed.
- Return bounded project-relative paths and aggregate counts; never return canonical roots or file contents.
- Report missing/ambiguous markers and unavailable live capabilities explicitly.

# Work Guidance

- Keep UEFN-specific classification here, outside RELAY Core.
- Integrate through the shared command path without bypassing Core authorization or registry validation.

# Verification

- Run a crate build check after adapter changes.

# Child DOX Index

- No child DOX files currently exist.
