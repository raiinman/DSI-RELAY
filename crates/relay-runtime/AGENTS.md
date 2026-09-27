# Purpose

Own the bounded runtime session and probe state model.

# Ownership

- Governs `crates/relay-runtime/`.
- Consumes normalized capture evidence from `relay-verse`; it does not attach to UEFN, Fortnite, project files, or storage.

# Local Contracts

- Enforce explicit disconnected, launching, connected, running, stopping, and ended transitions. A lost or aborted session cannot be silently reused as a clean session.
- Bind every tracker to one project ID, session ID, and declared source version. Reject capture context mismatches without mutating state.
- Keep probes bounded and project-defined. Snapshots contain compact counts and codes, never raw logs or player data.
- A complete capture can support local positive and negative assertions; an incomplete capture can only show positive witnesses. Neither a synthetic nor an imported capture proves live UEFN execution.
- Live UEFN outcome remains `untested` until a separate trusted adapter establishes provenance and a real session is exercised.

# Work Guidance

- Preserve capture completeness and provenance limits from `relay-verse`; never upgrade caller declarations to verified source identity.

# Verification

- Run focused state-invariant tests and compile the crate. The final integrated run requires real UEFN for live assertions.

# Child DOX Index

- No child DOX files currently exist.
