---
name: Measurement preview benchmark report
about: Share an opt-in synthetic benchmark result from a published RELAY preview
title: "Benchmark: Windows / CPU / RAM / storage class"
---

Use this template only with a publicly released RELAY measurement preview. **This issue and any attachment are public.** Read [the privacy statement](https://github.com/raiinman/DSI-RELAY/blob/main/PRIVACY.md) and inspect every field of your JSON report before attaching it. GitHub uploads an attachment as soon as you add it. You may submit the summary without attaching the report. Do not post project files, raw logs, account or computer names, paths, credentials, IP addresses, or screenshots containing them. Report suspected vulnerabilities through [the private security channel](https://github.com/raiinman/DSI-RELAY/security/policy).

## Preview and run

- Preview version:
- Downloaded ZIP SHA-256 (from the publisher's release page):
- Report `schema_version` and `benchmark_version`:
- Report `status` (`passed`, `failed`, or `interrupted`):
- `runs_completed` / `runs_requested`:
- `file_count_per_project`:
- If failed, `failure_code` and `current_stage` (no raw log):

## Hardware and conditions

<!-- Copy only the system classes you are comfortable sharing. The CPU model and Windows build can be identifying in combination with other details. Leave a field blank if you prefer. -->

- Windows version/build:
- CPU model and physical/logical core counts:
- Installed RAM:
- Temporary storage class (SSD/HDD/unknown):
- Power plan class:
- Was the machine plugged in or on battery?
- Other significant load during the run (for example, antivirus scan or creator app):
- Was this a normal desktop account or a virtual machine?

## Result and consent

- Did every completed sample have all `correctness` values set to `true`?
- Any unexpected behavior, interruption, or remaining RELAY process after the run?
- Optional: attach the JSON report **only after opening and reviewing it**. The benchmark does not upload it for you. If you choose not to attach it, the summary is still useful for recruitment and troubleshooting; timing comparisons need the raw samples.
- I reviewed what I am sharing and choose to make it public: yes / no

<!-- A 100-file smoke run checks basic operation. It does not establish a supported hardware tier. The synthetic benchmark does not measure real creator-app interference or long-run idle cost. -->
