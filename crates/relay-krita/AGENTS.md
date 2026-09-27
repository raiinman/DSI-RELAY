# Purpose

Own Krita-specific asset inspection and capability reporting outside RELAY Core.

# Ownership

- This crate interprets Krita source/export records from the generic `relay-assets` manifest.
- `relay-assets` owns project-relative path, file-link, and declared-lineage validation.
- RELAY Core, not this crate, owns command authority, jobs, transactions, and stored results.

# Local Contracts

- Local inspection is bounded and reads file metadata through `relay-assets`; it does not open source or export contents.
- `.kra` source and `.png` export checks validate declared filename formats only. They do not prove that Krita can load or export the files.
- Discover an explicitly configured Krita binary by metadata only. Do not search the machine, launch Krita, or return its path in reports.
- [Krita's CLI manual](https://docs.krita.org/en/reference_manual/linux_command_line.html) documents `krita <source> --export --export-filename <target>` and states CLI export is supported on Windows. Native export stays disabled until an authorized shared command executes and records it.
- Creator-app execution and asset-content validation remain `untested` in this crate's reports.

# Work Guidance

- Keep output compact, path-free, and deterministic. Do not include file bytes or user-specific locations.
- Do not infer a successful native workflow from a present binary, matching extensions, or a declared revision.

# Verification

- Compile this crate after the workspace adds it.

# Child DOX Index

- No child DOX files currently exist; this file owns the crate.
