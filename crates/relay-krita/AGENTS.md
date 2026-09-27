# Purpose

Own Krita-specific asset inspection and fixed local KRA-to-PNG export outside RELAY Core.

# Ownership

- This crate interprets Krita source/export records from the generic `relay-assets` manifest.
- `relay-assets` owns project-relative path, file-link, and declared-lineage validation.
- RELAY Core, not this crate, owns command authority, jobs, transactions, and stored results.

# Local Contracts

- Local inspection is bounded and reads file metadata through `relay-assets`; it does not open source or export contents.
- `.kra` source and `.png` export checks validate declared filename formats only. They do not prove that Krita can load or export the files.
- Metadata inspection still discovers only an explicitly configured binary without launching it or returning its path.
- Explicit export uses only fixed standard-install Krita candidates and [Krita's documented CLI](https://docs.krita.org/en/reference_manual/linux_command_line.html): `krita <source> --export --export-filename <target>`. No caller-selected executable or script is accepted.
- Export accepts a bounded regular `.kra`, stages a PNG with a random sibling name in the target directory, stops oversized output, checks PNG size and signature, and publishes through create-only same-volume linking. Existing targets are never overwritten; temporary files are cleaned by their exact path.
- Successful export records SHA-256 digests for the source and published PNG, with a second source digest check before publication. Digests are provenance observations, not full image-content validation.
- Export status distinguishes missing Krita (`untested`), interrupted work (`incomplete`), and a completed native attempt. A PNG signature is not full image-content validation, which remains `untested`.
- Recovery inspection reads an existing project-local KRA/PNG pair through the host's path gate and checks each opened Windows handle against the canonical project root, refusing reparse points and concurrent writers. It bounds both files, checks a PNG header, and hashes each handle twice without changing either file. Platforms without that handle guard report unavailable. Its result is a candidate with unverified native origin; only a fresh explicit export to a new path can establish native execution evidence.

# Work Guidance

- Keep output compact, path-free, and deterministic. Do not include file bytes or user-specific locations.
- Do not infer a successful native workflow from a present binary, matching extensions, or a declared revision. Only the explicit export process can report an exported PNG.

# Verification

- Focused crate checks and path/format tests; a real Krita export remains part of integrated testing.
- Recovery fixture checks verify unavailable and malformed files, bounded hashes, unchanged output, and unverified native-origin labels; they are not creator-app tests.

# Child DOX Index

- No child DOX files currently exist; this file owns the crate.
