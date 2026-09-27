# Purpose

Own the bounded, local asset manifest and deterministic source/export relationship checks.

# Ownership

- This file governs `crates/relay-assets/`.
- Blender and Krita execution belongs to future creator-app adapters; this crate records their declared asset provenance and validates local links only.

# Local Contracts

- Manifest data is project-relative, size bounded, and independent of creator applications.
- Reject traversal, links that escape the selected project root, symlinks/reparse points in asset file paths, duplicate identifiers, and inconsistent source/export lineage.
- Validation reports use stable codes and asset IDs without absolute paths or file contents.
- Impact analysis accepts at most 256 structurally safe changed project-relative paths against a structurally checked manifest, including declared files now missing or stale on disk. It treats related asset links as undirected dependencies and returns affected IDs in deterministic order with bounded source, export, target, or related-dependency reasons; it does not return paths.
- Never describe recorded relationships as proof of creator-app execution or actual mesh/texture correctness.

# Work Guidance

- Keep validation deterministic and cheap; do not read or hash asset contents during manifest checks.
- Keep impact analysis structural and in memory. Recheck path and relationship safety, but do not turn an impact result into a native-app validation claim.
- Add adapter-specific metadata only through explicit versioned schema changes.

# Verification

- Run focused crate tests when its manifest or path-safety contract changes.

# Child DOX Index

No child AGENTS.md files.
