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
  "2026-09-25-spike12-stable-windows-sandbox-matrix.json"
);
const spike11Path = path.join(
  phase1,
  "results",
  "2026-09-25-spike11-windows-adapter-sandbox.json"
);
const workerExe = path.join(
  rustRoot, "target", "release", "relay-sandbox-probe.exe"
);
const experimentalProbeExe = path.join(
  rustRoot, "target", "release", "relay-sandbox-broker-probe.exe"
);
const stableProbeExe = path.join(
  rustRoot, "target", "release", "relay-stable-sandbox-broker-probe.exe"
);
const coreExe = path.join(
  rustRoot, "target", "release", "relay-rust-challenger.exe"
);


function runJson(exe, args, timeout = 30000) {
  const result = spawnSync(exe, args, {
    cwd: rustRoot,
    encoding: "utf8",
    windowsHide: true,
    timeout
  });
  if (result.status !== 0) {
    throw new Error(
      exe + " " + args.join(" ") + " failed: " +
      (result.stderr || result.stdout)
    );
  }
  return JSON.parse(result.stdout.trim());
}

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


async function sourceMetrics() {
  const files = [
    path.join(rustRoot, "src", "stable_sandbox.rs"),
    path.join(rustRoot, "src", "sandbox_backend.rs"),
    path.join(
      rustRoot,
      "src",
      "bin",
      "relay-stable-sandbox-broker-probe.rs"
    )
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
  return {
    files: files.length,
    lines,
    bytes,
    unsafe_mentions: unsafeMentions
  };
}

const spike11 = JSON.parse(await fs.readFile(spike11Path, "utf8"));

const integration = spawnSync(
  "cargo",
  ["test", "--test", "spike12", "--", "--nocapture"],
  {
    cwd: rustRoot,
    windowsHide: true,
    encoding: "utf8",
    timeout: 30000
  }
);
if (integration.status !== 0) {
  throw new Error(
    "Spike 12 integration tests failed: " +
    (integration.stderr || integration.stdout)
  );
}


const buildMs = cleanReleaseBuild();
const stable = runJson(
  stableProbeExe,
  [workerExe, "30"],
  30000
);
const experimental = runJson(
  experimentalProbeExe,
  [workerExe, "30"],
  30000
);
const metadata = cargoMetadata();
const rootPackage = metadata.packages.find(
  pkg => pkg.name === "relay-rust-challenger"
);
const directDependencyCount = rootPackage?.dependencies.length ?? null;
const source = await sourceMetrics();

const coreBytes = (await fs.stat(coreExe)).size;
const workerBytes = (await fs.stat(workerExe)).size;
const stableProbeBytes = (await fs.stat(stableProbeExe)).size;
const experimentalProbeBytes = (await fs.stat(experimentalProbeExe)).size;

const spike11Build = spike11.build_and_complexity.spike11;
const stableP50 = stable.total.p50_ms;
const experimentalP50 = experimental.deny_total.p50_ms;


const result = {
  benchmark: "phase1-spike12-stable-windows-sandbox-matrix",
  recorded_at: new Date().toISOString(),
  decision_informed:
    "Which release-appropriate Windows sandbox backend/tier should enforce D-156 strong adapter isolation without depending on the experimental processmodel export.",
  hypothesis:
    "Documented LPAC AppContainer launch with temporary broker-owned ACL/Low-IL grants plus brokered egress can reproduce Spike 11's security guarantees at comparable cost, while unsupported or unmeasured Windows tiers fail closed.",
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
    total_memory_bytes: os.totalmem()
  },
  selected_backend: stable.backend_selection,
  measurements: {
    stable_lpac: {
      prelaunch: stable.prelaunch,
      create_process: stable.launch,
      full_restored_round_trip: stable.total,
      deny_result: stable.deny_result,
      direct_write_grant_count: stable.direct_write_grant_count,
      temporary_acl_grant_count: stable.temporary_acl_grant_count,
      temporary_low_il_label_count: stable.temporary_low_il_label_count,
      temporary_security_restored: stable.temporary_security_restored,
      job_limits: stable.job_limits,
      brokered_egress: stable.brokered_egress,
      denied_egress_error_code: stable.denied_egress_error_code,
      direct_capability_error_code: stable.direct_capability_error_code
    },
    experimental_reference: {
      launch: experimental.deny_launch,
      full_round_trip: experimental.deny_total,
      deny_result: experimental.deny_result,
      job_limits: experimental.job_limits
    },
    stable_minus_experimental_p50_ms:
      +(stableP50 - experimentalP50).toFixed(3),
    stable_vs_experimental_p50_ratio:
      +(stableP50 / experimentalP50).toFixed(4),
    host_network_baseline: stable.host_network_baseline
  },

  build_and_complexity: {
    spike11_reference: spike11Build,
    spike12: {
      clean_release_all_bins_ms: +buildMs.toFixed(3),
      direct_dependency_count: directDependencyCount,
      resolved_package_count: metadata.packages.length,
      core_release_binary_bytes: coreBytes,
      sandbox_worker_binary_bytes: workerBytes,
      stable_probe_binary_bytes: stableProbeBytes,
      experimental_probe_binary_bytes: experimentalProbeBytes,
      source_metrics: source
    },
    deltas_vs_spike11: {
      direct_dependencies:
        directDependencyCount - spike11Build.direct_dependency_count,
      resolved_packages:
        metadata.packages.length - spike11Build.resolved_package_count,
      core_release_binary_bytes:
        coreBytes - spike11Build.core_release_binary_bytes
    }
  },
  failure_evidence: {
    integration_tests_exit_code: integration.status,
    integration_test_count: 3,
    simulated_matrix: stable.simulated_matrix,
    unsupported_direct_capability:
      stable.direct_capability_error_code,
    denied_broker_egress:
      stable.denied_egress_error_code
  },

  pass_fail: {
    integration_tests_pass: integration.status === 0,
    stable_backend_selected:
      stable.backend_selection.selected_backend ===
        "stable_lpac_brokered_egress" &&
      stable.backend_selection.strong_untrusted_launch_enabled === true,
    measured_build_selected:
      stable.backend_selection.measured_build === true,
    experimental_not_release_fallback:
      stable.simulated_matrix.experimental_only.selected_backend ===
        "disabled" &&
      stable.simulated_matrix.experimental_only
        .strong_untrusted_launch_enabled === false,
    unmeasured_build_fails_closed:
      stable.simulated_matrix.unmeasured_stable.selected_backend ===
        "disabled" &&
      stable.simulated_matrix.unmeasured_stable
        .strong_untrusted_launch_enabled === false,
    stable_without_experimental_still_selected:
      stable.simulated_matrix.measured_stable_without_experimental
        .selected_backend === "stable_lpac_brokered_egress" &&
      stable.simulated_matrix.measured_stable_without_experimental
        .strong_untrusted_launch_enabled === true,
    appcontainer_verified: stable.deny_result.is_app_container === true,
    allowed_mailbox_read_works:
      stable.deny_result.allowed_read_ok === true,
    allowed_mailbox_write_works:
      stable.deny_result.allowed_write_ok === true,
    read_only_worker_write_denied:
      stable.deny_result.read_only_write_ok === false,
    ungranted_file_read_denied:
      stable.deny_result.blocked_read_ok === false,
    ungranted_file_write_denied:
      stable.deny_result.blocked_write_ok === false,
    direct_worker_network_denied:
      stable.deny_result.network_connect_ok === false,
    parent_secret_not_inherited:
      stable.deny_result.parent_secret_visible === false,
    userprofile_not_inherited:
      stable.deny_result.user_profile_visible === false,
    child_process_creation_denied:
      stable.deny_result.child_process_created === false,

    outer_job_active_process_limit:
      stable.job_limits.active_process_limit === 1,
    outer_job_memory_limit:
      stable.job_limits.process_memory_limit_bytes === 32 * 1024 * 1024,
    outer_job_kill_on_close:
      stable.job_limits.kill_on_close === true,
    temporary_security_restored:
      stable.temporary_security_restored === true,
    mailbox_is_only_direct_write_grant:
      stable.direct_write_grant_count === 1,
    direct_capability_grant_rejected:
      stable.direct_capability_error_code ===
        "STABLE_SANDBOX_DIRECT_CAPABILITY_UNSUPPORTED",
    brokered_egress_policy_enforced:
      stable.denied_egress_error_code ===
        "STABLE_SANDBOX_EGRESS_DENIED",
    brokered_egress_works:
      stable.host_network_baseline === true &&
      stable.brokered_egress.allowed_by_policy === true &&
      stable.brokered_egress.connect_ok === true,
    experimental_reference_still_passes:
      experimental.deny_result.is_app_container === true &&
      experimental.deny_result.blocked_read_ok === false &&
      experimental.deny_result.blocked_write_ok === false &&
      experimental.deny_result.network_connect_ok === false,
    stable_p50_under_200ms: stable.total.p50_ms < 200,
    stable_p95_under_300ms: stable.total.p95_ms < 300,
    stable_p50_under_2x_experimental:
      stableP50 / experimentalP50 < 2,
    no_new_direct_dependency_vs_spike11:
      directDependencyCount === spike11Build.direct_dependency_count,
    resolved_package_growth_bounded:
      metadata.packages.length - spike11Build.resolved_package_count <= 1,
    core_binary_growth_bounded:
      coreBytes - spike11Build.core_release_binary_bytes < 512 * 1024
  },

  limitations: [
    "The stable AppContainer APIs are documented back to Windows 8 desktop apps, but RELAY only enables strong untrusted launch on Windows builds that have passed the full adversarial fixture; this prototype allowlists build 26200 only.",
    "The stable backend intentionally gives the untrusted worker exactly one direct read/write grant: a broker-owned ephemeral mailbox. Project/user writes must return through trusted broker commands rather than direct worker filesystem authority.",
    "The stable backend temporarily grants AppContainer DACL access to the mailbox/worker tree and applies Low Integrity only to the ephemeral mailbox. Tests verify the DACL is restored, the Low-Integrity label is removed after exit, and the worker tree descriptor is restored exactly.",
    "Direct worker network capabilities are intentionally disabled in the stable backend. Network access is brokered through an explicit target allowlist, so future real adapters may need a higher-level HTTP/RPC egress service rather than raw sockets.",
    "The positive brokered-egress check uses external TCP reachability to 1.1.1.1:443 and is fixture/network dependent; the benchmark separately requires host baseline reachability.",
    "The experimental processmodel backend remains in the repository only as a measured reference and is never selected by the release backend matrix.",
    "Real UEFN, Blender, Krita, and other developer-tool adapter compatibility has not been tested inside the stable LPAC boundary.",
    "Only one Windows 11 build and one high-end workstation were physically measured; additional Windows builds require the same adversarial fixture before being added to the release allowlist."
  ]
};

await fs.mkdir(path.dirname(resultPath), { recursive: true });
await fs.writeFile(
  resultPath,
  JSON.stringify(result, null, 2) + "\n",
  "utf8"
);
console.log(JSON.stringify(result, null, 2));
