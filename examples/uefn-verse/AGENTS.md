# Purpose

Own a minimal Verse device example that emits structured RELAY session markers and one device startup probe.

# Ownership

- This file governs `examples/uefn-verse/`.
- `crates/relay-verse/` owns capture parsing and evidence quality; this example only writes diagnostic messages from Verse.

# Local Contracts

- Emit `RELAY_EVENT_V1 ` followed by one bounded JSON object per event with schema version 1, a session ID, strictly increasing sequence, relative milliseconds, and a token event kind.
- Emit only one `session_start`, one project-defined probe, and one synchronous `session_end` from `OnEnd`. An absent end marker leaves a capture incomplete.
- Use Epic's supported Verse diagnostics logger at Normal level; no sockets, gameplay automation, player data, or project-specific identifiers.
- UEFN compilation, log capture, and play-session behavior are UNTESTED until a real UEFN run confirms them. Engine log decoration must be removed before passing event lines to the exact-prefix RELAY parser.

# Work Guidance

- Keep editable values numeric so interpolated JSON cannot contain unescaped user text.
- Keep output and setup examples privacy-safe and project-agnostic.

# Verification

- Compare emitted object fields and limits with `crates/relay-verse/src/lib.rs`. UEFN compile and live output remain separate acceptance checks.

# Child DOX Index

No child AGENTS.md files.
