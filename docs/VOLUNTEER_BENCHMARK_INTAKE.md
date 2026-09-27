# Volunteer measurement-preview reports

Use the [benchmark issue template](../.github/ISSUE_TEMPLATE/benchmark-report.md) only after an authorized public measurement preview exists. Reports are submitted manually. RELAY does not upload telemetry, and this process needs no account or service beyond a volunteer's chosen GitHub issue. Public issues and attachments may be copied or indexed, so the volunteer must inspect the JSON and decide what to share. The [privacy statement](../PRIVACY.md) describes the report fields; suspected vulnerabilities belong in the [private security channel](../SECURITY.md).

## What to ask volunteers to run

Start with the published bundle's instructions and verifier. A short 100-file run is functional smoke. For exploratory hardware comparisons, use the same preview version, fixture size, and run count across machines; request the raw JSON samples where volunteers consent. The portable runner supports 100–7,500 files per project and 1–20 runs. A 7,500-file, 20-run report can contribute to a tier study, but the synthetic fixture alone cannot pass the Phase 3 resource gates. Volunteers should note battery/AC power, competing work, and whether the machine is a virtual machine. Do not ask them to open private projects for this benchmark.

## Review each issue

1. Check the stated preview version and ZIP SHA-256 against the published release, then check `schema_version`, `benchmark_version`, fixture, file count, and requested/completed run counts. Treat an absent or altered report as a summary, not raw timing evidence.
2. Treat a `passed` report as a valid synthetic observation only when all requested samples completed and every sample's `correctness` values are true. A `failed` or `interrupted` report is useful for troubleshooting, but its timings cannot support a tier claim. Use `failure_code` and `current_stage`; ask for no raw project data or credentials.
3. Compare raw samples only within the same preview build, fixture size, storage/power class, and declared test conditions. Keep individual samples and outliers visible. Do not pool a virtual machine with an ordinary desktop tier or infer a slow-PC pass from a fast machine under artificial throttling.
4. If a report exposes a name, path, token, or other private detail, ask the author to remove the attachment or edit the issue. Do not quote that detail in a response or copy it into a public aggregate. Do not download unnecessary attachments.
5. Record the issue link and coarse machine class in the Phase 3 evidence review. Mark it as **functional smoke**, **exploratory timing**, **troubleshooting**, or **candidate tier evidence**. A candidate tier still needs the repeat count, numerical limits, sustained idle/recovery measurement, and real creator-app foreground test in the [resource-budget method](PHASE3_RESOURCE_BUDGET_METHOD.md).

Do not represent volunteer issue volume or a hosted CI run as the number of distinct physical machines tested. Publish supported hardware claims only after the Phase 3 gates are met.
