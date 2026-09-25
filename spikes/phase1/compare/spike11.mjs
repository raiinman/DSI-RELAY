import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { execFileSync, spawnSync } from "node:child_process";
import { performance } from "node:perf_hooks";

const here = path.resolve(import.meta.dirname);
const phase1 = path.resolve(here, "..");
const rustRoot = path.join(phase1, "rust");
const resultPath = path.join(
  phase1,
  "results",
  "2026-09-25-spike11-windows-adapter-sandbox.json"
);
const spike10Path = path.join(
  phase1,
  "results",
  "2026-09-24-spike10-adapter-broker-windows.json"
);
const workerExe = path.join(
  rustRoot,
  "target",
  "release",
  "relay-sandbox-probe.exe"
);
const probeExe = path.join(
  rustRoot,
  "target",
  "release",
  "relay-sandbox-broker-probe.exe"
);
const coreExe = path.join(
  rustRoot,
  "target",
  "release",
  "relay-rust-challenger.exe"
);

function cleanReleaseBuild() {
  execFileSync("cargo", ["clean", "--release"], {
    cwd: rustRoot,
    windowsHide: true,
    stdio: "ignore"
  });
  const started = performance.now();
  execFileSync("cargo", ["build", "--release", "--bins"], {
    cwd: rustRoot,
    windowsHide: true,
    stdio: "ignore"
  });
  return performance.now() - started;
}


function cargoMetadata() {
  return JSON.parse(execFileSync(
    "cargo",
    ["metadata", "--format-version", "1", "--locked"],
    { cwd: rustRoot, encoding: "utf8", windowsHide: true }
  ));
}

function runProbe(iterations = 30) {
  const result = spawnSync(
    probeExe,
    [workerExe, String(iterations)],
    {
      cwd: rustRoot,
      encoding: "utf8",
      windowsHide: true,
      timeout: 30000
    }
  );
  if (result.status !== 0) {
    throw new Error(
      "sandbox probe failed: " + (result.stderr || result.stdout)
    );
  }
  return JSON.parse(result.stdout.trim());
}

function processmodelVersion() {
  const script =
    "$f=Get-Item \"$env:SystemRoot\\System32\\processmodel.dll\"; " +
    "[pscustomobject]@{version=$f.VersionInfo.FileVersion;bytes=$f.Length} | " +
    "ConvertTo-Json -Compress";
  return JSON.parse(execFileSync(
    "powershell.exe",
    ["-NoProfile", "-Command", script],
    { encoding: "utf8", windowsHide: true }
  ).trim());
}

async function sourceMetrics() {
  const files = [
    path.join(rustRoot, "src", "sandbox.rs"),
    path.join(rustRoot, "src", "bin", "relay-sandbox-probe.rs")
  ];
  let lines = 0;
  let bytes = 0;
  let unsafeMentions = 0;
  for (const file of files) {
    const text = await fs.readFile(file, "utf8");
    lines += text.split(/\r?\n/).length;
    bytes += Buffer.byteLength(text);
    unsafeMentions += (text.match(/\bunsafe\b/g) ?? []).length;
  }
  return { files: files.length, lines, bytes, unsafe_mentions: unsafeMentions };
}


const spike10 = JSON.parse(await fs.readFile(spike10Path, "utf8"));

const integration = spawnSync(
  "cargo",
  ["test", "--test", "spike11", "--", "--nocapture"],
  {
    cwd: rustRoot,
    windowsHide: true,
    encoding: "utf8",
    timeout: 30000
  }
);
if (integration.status !== 0) {
  throw new Error(
    "Spike 11 integration tests failed: " +
    (integration.stderr || integration.stdout)
  );
}

const buildMs = cleanReleaseBuild();
const probe = runProbe(30);
const metadata = cargoMetadata();
const rootPackage = metadata.packages.find(
  pkg => pkg.name === "relay-rust-challenger"
);
const directDependencyCount = rootPackage?.dependencies.length ?? null;
const source = await sourceMetrics();
const processmodel = processmodelVersion();

const coreBytes = (await fs.stat(coreExe)).size;
const workerBytes = (await fs.stat(workerExe)).size;
const probeBytes = (await fs.stat(probeExe)).size;

const spike10Build = spike10.build_and_complexity.spike10;
const unsandboxedP50 = spike10.measurements.normal_invocation.p50_ms;
const sandboxedP50 = probe.deny_total.p50_ms;

const result = {
  benchmark: "phase1-spike11-windows-adapter-sandbox",
  recorded_at: new Date().toISOString(),
  decision_informed:
    "Which Windows OS-enforced capability/egress boundary should RELAY use for synthetic untrusted adapter workers after Spike 10 established the broker/Job Object foundation.",
  hypothesis:
    "Windows AppContainer/process sandboxing with explicit filesystem grants, default-deny network, broker-allowlisted capabilities, a sanitized environment, and outer Job Object limits can strongly restrict synthetic adapter workers while preserving a narrow mailbox data path.",
  runtime: {
    os: { platform: process.platform, release: os.release(), arch: os.arch() },
    rustc: execFileSync("rustc", ["--version"], {
      encoding: "utf8", windowsHide: true
    }).trim(),
    cargo: execFileSync("cargo", ["--version"], {
      encoding: "utf8", windowsHide: true
    }).trim(),
    cpu_model: os.cpus()[0]?.model.trim() ?? "unknown",
    logical_cpu_count: os.cpus().length,
    total_memory_bytes: os.totalmem(),
    processmodel_dll: processmodel
  },

  selected_boundary: {
    backend:
      "Experimental_CreateProcessInSandbox from processmodel.dll on the measured Windows fixture",
    sandbox_spec_version: "0.1.0",
    app_container: true,
    filesystem: {
      mailbox: "explicit read/write grant",
      worker_directory: "explicit read-only grant",
      outside_grants: "default deny"
    },
    network: {
      default: "deny",
      granted:
        "broker allowlist + internetClient token capability + SandboxSpec egress default-allow"
    },
    environment:
      "custom Unicode environment block; no inherited handles; synthetic parent secret and USERPROFILE omitted",
    process: {
      outer_job_active_process_limit: 1,
      outer_job_memory_limit_bytes: 33554432,
      outer_job_kill_on_close: true
    },
    fallback: "fail closed; never silently launch unrestricted"
  },

  measurements: {
    api_available: probe.api_available,
    deny_launch: probe.deny_launch,
    deny_total: probe.deny_total,
    unsandboxed_spike10_invocation_p50_ms: unsandboxedP50,
    sandbox_overhead_p50_ms:
      +(sandboxedP50 - unsandboxedP50).toFixed(3),
    sandbox_vs_unsandboxed_p50_ratio:
      +(sandboxedP50 / unsandboxedP50).toFixed(4),
    deny_result: probe.deny_result,
    job_limits: probe.job_limits,
    host_network_baseline: probe.host_network_baseline,
    network_grant: probe.network_grant,
    invalid_spec_error_code: probe.invalid_spec_error_code,
    unknown_capability_error_code: probe.unknown_capability_error_code
  },

  build_and_complexity: {
    spike10_baseline: {
      direct_dependency_count: spike10Build.direct_dependency_count,
      resolved_package_count: spike10Build.resolved_package_count,
      core_release_binary_bytes: spike10Build.core_release_binary_bytes
    },
    spike11: {
      clean_release_all_bins_ms: +buildMs.toFixed(3),
      direct_dependency_count: directDependencyCount,
      resolved_package_count: metadata.packages.length,
      core_release_binary_bytes: coreBytes,
      sandbox_worker_binary_bytes: workerBytes,
      sandbox_probe_binary_bytes: probeBytes,
      source_metrics: source
    },
    deltas_vs_spike10: {
      direct_dependencies:
        directDependencyCount - spike10Build.direct_dependency_count,
      resolved_packages:
        metadata.packages.length - spike10Build.resolved_package_count,
      core_release_binary_bytes:
        coreBytes - spike10Build.core_release_binary_bytes
    }
  },

  failure_evidence: {
    integration_tests_exit_code: integration.status,
    integration_test_count: 4,
    unsupported_spec:
      probe.invalid_spec_error_code,
    unsupported_capability:
      probe.unknown_capability_error_code,
    exploratory_platform_quirks: [
      "On this fixture, internetClient capability alone reached the AppContainer token but raw TCP still failed with WSAEACCES 10013; adding SandboxSpec egress default-allow produced the explicit grant path used by the final probe.",
      "During exploratory probing, Windows accepted an unknown capability string instead of failing launch as expected; the final RELAY path therefore allowlists supported capability names before calling the experimental API."
    ]
  },

  pass_fail: {
    integration_tests_pass: integration.status === 0,
    experimental_api_available: probe.api_available === true,
    appcontainer_verified: probe.deny_result.is_app_container === true,
    allowed_mailbox_read_works: probe.deny_result.allowed_read_ok === true,
    allowed_mailbox_write_works: probe.deny_result.allowed_write_ok === true,
    read_only_worker_write_denied:
      probe.deny_result.read_only_write_ok === false,
    ungranted_file_read_denied:
      probe.deny_result.blocked_read_ok === false,
    ungranted_file_write_denied:
      probe.deny_result.blocked_write_ok === false,
    default_network_denied:
      probe.deny_result.network_connect_ok === false,
    default_network_denied_by_os:
      probe.deny_result.network_error_code === 10013,
    parent_secret_not_inherited:
      probe.deny_result.parent_secret_visible === false,
    userprofile_not_inherited:
      probe.deny_result.user_profile_visible === false,
    child_process_creation_denied:
      probe.deny_result.child_process_created === false,
    outer_job_active_process_limit:
      probe.job_limits.active_process_limit === 1,
    outer_job_memory_limit:
      probe.job_limits.process_memory_limit_bytes === 33554432,
    outer_job_kill_on_close:
      probe.job_limits.kill_on_close === true,
    host_network_baseline_available:
      probe.host_network_baseline === true,
    explicit_network_grant_works:
      probe.network_grant.result.network_connect_ok === true,
    network_grant_keeps_filesystem_denied:
      probe.network_grant.result.blocked_read_ok === false &&
      probe.network_grant.result.blocked_write_ok === false &&
      probe.network_grant.result.read_only_write_ok === false,
    network_grant_keeps_child_process_denied:
      probe.network_grant.result.child_process_created === false,
    incompatible_spec_fails_closed:
      probe.invalid_spec_error_code === "SANDBOX_SPEC_INCOMPATIBLE",
    unknown_capability_fails_in_relay:
      probe.unknown_capability_error_code === "SANDBOX_CAPABILITY_UNSUPPORTED",
    sandbox_p50_under_150ms:
      probe.deny_total.p50_ms < 150,
    sandbox_p95_under_250ms:
      probe.deny_total.p95_ms < 250,
    dependency_growth_bounded:
      directDependencyCount - spike10Build.direct_dependency_count <= 1 &&
      metadata.packages.length - spike10Build.resolved_package_count <= 4,
    core_binary_growth_bounded:
      coreBytes - spike10Build.core_release_binary_bytes <= 512 * 1024
  },

  limitations: [
    "Experimental_CreateProcessInSandbox is explicitly experimental Microsoft API surface and may change or disappear; D-156 must not treat the export itself as a permanent public RELAY compatibility promise.",
    "The selected security semantics are stronger than Spike 10, but real UEFN/Blender/Krita adapter compatibility has not been tested inside this AppContainer/process sandbox.",
    "The mailbox filesystem channel is a synthetic proof that a narrow brokered data path remains usable; it is not yet the final adapter transport.",
    "The positive network grant uses external TCP reachability to 1.1.1.1:443 and is therefore fixture/network dependent; the benchmark separately verifies host baseline reachability.",
    "Only internetClient is allowlisted in the Spike 11 RELAY sandbox policy. Additional Windows capability names require their own evidence before use.",
    "Filesystem isolation behavior is measured on this Windows build; Windows process-container implementations may use different underlying enforcement/fallback mechanisms across supported OS tiers.",
    "The worker still depends on Windows system libraries available to AppContainer and on a sanitized set of standard Windows environment variables.",
    "Only one high-end Windows workstation was benchmarked."
  ]
};

const failed = Object.entries(result.pass_fail)
  .filter(([, value]) => !value)
  .map(([key]) => key);
if (failed.length) {
  throw new Error("Spike 11 gate failed: " + failed.join(", "));
}

await fs.mkdir(path.dirname(resultPath), { recursive: true });
await fs.writeFile(
  resultPath,
  JSON.stringify(result, null, 2) + "\n",
  "utf8"
);
console.log(JSON.stringify(result, null, 2));
