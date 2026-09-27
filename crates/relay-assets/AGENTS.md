# Purpose

Own the bounded, local asset manifest and deterministic source/export relationship checks.

# Ownership

- This file governs `crates/relay-assets/`.
- Blender and Krita execution belongs to future creator-app adapters; this crate records their declared asset provenance and validates local links only.

# Local Contracts

- Manifest data is project-relative, size bounded, and independent of creator applications.
- Reject traversal, links that escape the selected project root, symlinks/reparse points in asset file paths, duplicate identifiers, and inconsistent source/export lineage.
- Validation reports use stable codes and asset IDs without absolute paths or file contents.
- Never describe recorded relationships as proof of creator-app execution or actual mesh/texture correctness.

# Work Guidance

- Keep validation deterministic and cheap; do not read or hash asset contents during manifest checks.
- Add adapter-specific metadata only through explicit versioned schema changes.

# Verification

- Run focused crate tests when its manifest or path-safety contract changes.

# Child DOX Index

No child AGENTS.md files.
