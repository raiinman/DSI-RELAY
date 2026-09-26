# Purpose

Own Windows-native Phase 1 helper source used to benchmark operating-system scheduling, QoS, Job Object, and window-responsiveness behavior.

# Ownership

- This file governs `spikes/phase1/node/windows/`.
- Parent Node and Phase 1 DOX contracts remain authoritative.

# Local Contracts

- Commit source only. Compiled EXE/DLL artifacts must be generated into temporary directories and deleted after the benchmark.
- Do not require administrator elevation, disable security controls, or change persistent PowerShell execution policy for a spike.
- Resource controls may be applied only to RELAY-owned benchmark/worker processes; never raise, lower, suspend, cap, or otherwise modify UEFN/Fortnite/Blender/Krita process policy.
- Native helpers must query back applied state so benchmark results distinguish requested controls from controls the OS actually accepted.
- Hard caps, power throttling, priority, and memory-priority behavior remain benchmark inputs rather than production defaults.

# Work Guidance

- Keep P/Invoke surfaces narrow and explicit.
- Prefer documented Windows APIs.
- Treat unavailable/unsupported controls as measured results rather than bypassing the operating system.

# Verification

- Compile helper source to a temporary output directory during closeout.
- Verify no `.exe` or `.dll` exists under this repository subtree after testing.

# Child DOX Index

- No child DOX files currently exist under this folder.
