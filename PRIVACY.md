# Privacy for the measurement preview

The Windows measurement preview runs a synthetic file-indexing benchmark locally. Its runner creates temporary sample projects and a versioned JSON report at a path the tester chooses. It does not select existing project folders, send the report, or upload telemetry. A tester decides whether to share the report. During each run, the daemon opens its local named pipe and a loopback-only HTTP dashboard listener on `127.0.0.1`; the runner makes no remote network call.

The report includes system and performance information such as Windows version, CPU model/core count, installed memory, storage and power classes when available, binary hashes, sample counts, timings, memory use, and correctness results. The runner is designed to omit account and computer names, project paths and content, credentials, and local IPC tokens. Review the JSON before sharing it, especially when other system software may affect the measurements.

The benchmark starts a local RELAY daemon, then stops it and removes only its own temporary fixture directories during normal completion and handled failures. A hard interruption can leave a daemon or temporary fixture behind; the bundled clean-account checklist explains how to check. Removing the package folder does not remove a report written elsewhere.

This statement applies to the synthetic measurement preview, not future RELAY integrations or third-party tools. See [the preview plan](docs/PUBLIC_PREVIEW_PATH.md) for its limits.
