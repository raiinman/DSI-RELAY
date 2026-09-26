# Phase 2 third slice

Status: Complete — accepted on 2026-09-25. Evidence: `PHASE2_THIRD_SLICE_EVIDENCE.md`.

Goal: promote the generic adapter manifest/broker lifecycle and the D-155 through D-157 strong Windows worker-isolation boundary into production Rust interfaces.

The slice must prove together:
- generic versioned adapter manifests with publisher/provenance/component inventory
- requested permissions remain requests and fail closed unless broker policy grants them
- worker artifact and worker-component SHA-256 verification before install and again before invocation
- adapter command/version/capability bindings resolve through the shared RELAY registry
- on-demand out-of-process workers only; inactive adapters leave no resident worker
- stable AppContainer/LPAC launch on qualified Windows builds and fail-closed unqualified builds
- mailbox-only direct write access, read/execute-only worker package access, and default-deny direct network/process escape
- Job Object one-process, kill-on-close, and process-memory enforcement
- exact worker identity/version/protocol/capability and result/error schema validation
- bounded crash/hang backoff and quarantine with mailbox cleanup
- temporary ACL/Low-IL grants restore exactly after invocation
- same-package invocations serialize temporary security mutation while different packages remain independent
- dedicated adapter package directories prevent recursive ACL grants from touching shared build/repository trees
- resource use remains within the Phase 1 adapter/sandbox order of magnitude

Acceptance:
1. no real tool-specific behavior enters the generic crate;
2. manifest integrity and every permission class fail closed;
3. unmeasured Windows builds cannot launch untrusted workers;
4. the synthetic adversarial worker cannot read/write blocked paths, use direct network, inherit parent secrets/USERPROFILE, create children, or exceed the memory cap;
5. worker response identity/protocol/capability/result/error contracts fail closed;
6. crash, hang, invalid response, backoff, quarantine, uninstall, and post-install artifact tamper behavior are tested;
7. package security descriptors restore exactly, including concurrent same-package calls;
8. inactive installed-adapter overhead and full sandbox invocation cost are measured.
No UEFN, Fortnite, Blender, Krita, or other real tool adapter behavior belongs in this slice.

Phase 1 sandbox code remains evidence/reference only. The production crate intentionally promotes the documented stable AppContainer/LPAC path and does not ship the experimental processmodel backend.
