# Purpose

Own the bounded Blender headless mesh inspection adapter.

# Ownership

- Governs `crates/relay-blender/`.
- Blender-specific process and mesh behavior stays outside RELAY Core.

# Local Contracts

- Discovery checks explicit paths, PATH, and common installation folders without launching Blender.
- Validation accepts only a local `.blend` file and invokes Blender directly without a shell or caller-supplied script.
- Launch Blender in background with automatic script execution disabled before the file is opened. The fixed embedded script only reads mesh data; it never saves the file.
- Bound file size, process time, captured output, mesh count, and geometry inspection. Limit hits report incomplete, never passed.
- Treat native process output as untrusted: accept one bounded result with a sanitized version, sequential bounded mesh indices/counts, allowlisted issue codes, and internally consistent issue and completeness counts. Reject malformed output with a fixed error message and no raw logs.
- A detected executable is only a candidate. An unavailable executable or absent creator app session is never reported as a passed workflow.

# Work Guidance

- Keep results compact and avoid returning raw Blender logs or mesh names by default.
- Treat untrusted `.blend` content as unverified input; live validation requires a supported Blender installation.

# Verification

- Build and run focused crate tests. Full Blender validation remains untested until Blender is present.

# Child DOX Index

- No child DOX files currently exist.
