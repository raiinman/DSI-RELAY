import fs from "node:fs/promises";
import http from "node:http";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { performance } from "node:perf_hooks";
import { callHostWithState } from "../node/src/client.mjs";
import { attachJsonLines, sendJson } from "../node/src/wire.mjs";

const here = path.resolve(import.meta.dirname);
const phase1 = path.resolve(here, "..");
const rustRoot = path.join(phase1, "rust");
const rustExe = path.join(rustRoot, "target", "release", "relay-rust-challenger.exe");
const registryPath = path.join(phase1, "contracts", "commands.registry.json");
const spike8Path = path.join(
  phase1, "results", "2026-09-24-spike8-rust-storage-parity-windows.json"
);
const resultPath = path.join(
  phase1, "results", "2026-09-24-spike9-command-registry-windows.json"
);
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

function percentile(values, p) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(
    sorted.length - 1,
    Math.ceil((p / 100) * sorted.length) - 1
  )];
}

function summarize(values) {
  return {
    n: values.length,
    min_ms: +Math.min(...values).toFixed(3),
    p50_ms: +percentile(values, 50).toFixed(3),
    p95_ms: +percentile(values, 95).toFixed(3),
    p99_ms: +percentile(values, 99).toFixed(3),
    max_ms: +Math.max(...values).toFixed(3),
    mean_ms: +(values.reduce((sum, value) => sum + value, 0) / values.length).toFixed(3)
  };
}

function byteLength(value) {
  return Buffer.byteLength(typeof value === "string" ? value : JSON.stringify(value));
}

function psJson(command) {
  return JSON.parse(execFileSync(
    "powershell.exe",
    ["-NoProfile", "-Command", command],
    { encoding: "utf8", windowsHide: true }
  ).trim());
}

function processMetrics(pid) {
  return psJson(
    "$p=Get-Process -Id " + pid + " -ErrorAction Stop; " +
    "[pscustomobject]@{" +
    "cpu_ms=$p.TotalProcessorTime.TotalMilliseconds;" +
    "rss_bytes=[int64]$p.WorkingSet64;" +
    "private_bytes=[int64]$p.PrivateMemorySize64;" +
    "handle_count=$p.HandleCount;thread_count=$p.Threads.Count" +
    "} | ConvertTo-Json -Compress"
  );
}

async function idleSample(pid, sampleMs = 5000) {
  await sleep(500);
  const before = processMetrics(pid);
  await sleep(sampleMs);
  const after = processMetrics(pid);
  return {
    sample_ms: sampleMs,
    cpu_ms: +(after.cpu_ms - before.cpu_ms).toFixed(3),
    rss_bytes: after.rss_bytes,
    private_bytes: after.private_bytes,
    rss_delta_bytes: after.rss_bytes - before.rss_bytes,
    handle_count: after.handle_count,
    thread_count: after.thread_count
  };
}

async function waitForState(statePath, expectedPid, timeoutMs = 5000) {
  const started = performance.now();
  while (performance.now() - started < timeoutMs) {
    try {
      const state = JSON.parse(await fs.readFile(statePath, "utf8"));
      if (state.pid === expectedPid) {
        return { state, elapsed_ms: performance.now() - started };
      }
    } catch {}
    await sleep(5);
  }
  throw new Error("host state did not become current");
}

async function startHost(tempRoot) {
  const stateDir = path.join(tempRoot, "host");
  await fs.mkdir(stateDir, { recursive: true });
  const env = {
    ...process.env,
    RELAY_STATE_DIR: stateDir,
    RELAY_INSTANCE: "spike9-" + process.pid
  };
  const started = performance.now();
  const child = spawn(rustExe, ["host", "--dashboard"], {
    cwd: rustRoot,
    env,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"]
  });
  const ready = await waitForState(
    path.join(stateDir, "host-rust.json"),
    child.pid
  );
  return {
    child, state: ready.state, env,
    startup_ms: performance.now() - started
  };
}

async function stopHost(host) {
  if (host.child.exitCode !== null) return;
  const response = await callCommand(host.state, "system.shutdown", {});
  if (!response.ok) throw new Error("shutdown failed");
  await new Promise(resolve => host.child.once("close", resolve));
}

function callCommand(state, command, args = {}, commandVersion = undefined) {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection(state.pipe);
    let greeted = false;
    const requestId = "spike9-" + process.pid + "-" + performance.now();
    const timer = setTimeout(() => {
      socket.destroy();
      reject(new Error("command timeout"));
    }, 3000);

    socket.once("error", reject);
    attachJsonLines(socket, message => {
      if (!greeted) {
        if (message.type !== "hello_ok") {
          clearTimeout(timer);
          socket.destroy();
          return reject(new Error("handshake failed: " + JSON.stringify(message)));
        }
        greeted = true;
        const request = {
          type: "command",
          request_id: requestId,
          schema_version: 1,
          command,
          arguments: args
        };
        if (commandVersion !== undefined) request.command_version = commandVersion;
        sendJson(socket, request);
        return;
      }
      if (message.type === "command_result" && message.request_id === requestId) {
        clearTimeout(timer);
        socket.end();
        resolve(message);
      }
    }, reject);

    socket.once("connect", () => sendJson(socket, {
      type: "hello",
      auth_token: state.auth_token,
      protocol_min: 1,
      protocol_max: 1,
      client: { name: "spike9-neutral", version: "1" }
    }));
  });
}

async function measureCommand(state, command, args, count = 300) {
  for (let i = 0; i < 10; i++) {
    await callCommand(state, command, args);
  }
  const times = [];
  for (let i = 0; i < count; i++) {
    const started = performance.now();
    await callCommand(state, command, args);
    times.push(performance.now() - started);
  }
  return summarize(times);
}

function spawnRust(args, env) {
  return spawnSync(rustExe, args, {
    cwd: rustRoot,
    env,
    windowsHide: true,
    encoding: "utf8",
    timeout: 5000
  });
}

function cargoMetadata() {
  return JSON.parse(execFileSync(
    "cargo",
    ["metadata", "--format-version", "1", "--locked"],
    { cwd: rustRoot, encoding: "utf8", windowsHide: true }
  ));
}

function installedTool(name) {
  const command =
    "$c=Get-Command '" + name.replaceAll("'", "''") +
    "' -ErrorAction SilentlyContinue; if($c){$c.Source; exit 0}else{exit 0}";
  const result = spawnSync(
    "powershell.exe",
    ["-NoProfile", "-Command", command],
    { encoding: "utf8", windowsHide: true }
  );
  return result.stdout.trim() || null;
}

function cleanReleaseBuild() {
  execFileSync("cargo", ["clean", "--release"], {
    cwd: rustRoot, windowsHide: true, stdio: "ignore"
  });
  const started = performance.now();
  execFileSync("cargo", ["build", "--release"], {
    cwd: rustRoot, windowsHide: true, stdio: "ignore"
  });
  return performance.now() - started;
}

const tempRoot = await fs.mkdtemp(path.join(os.tmpdir(), "relay-spike9-"));
let host;
try {
  const spike8 = JSON.parse(await fs.readFile(spike8Path, "utf8"));
  const registrySource = await fs.readFile(registryPath, "utf8");
  const registryJson = JSON.parse(registrySource);
  const registryMinified = JSON.stringify(registryJson);

  const buildMs = cleanReleaseBuild();
  const binaryBytes = (await fs.stat(rustExe)).size;
  const metadata = cargoMetadata();
  const rootPackage = metadata.packages.find(
    pkg => pkg.name === "relay-rust-challenger"
  );
  const directDependencyCount = rootPackage?.dependencies.length ?? null;

  host = await startHost(tempRoot);
  const startupMs = host.startup_ms;
  const statusLatency = await measureCommand(
    host.state, "system.status", {}, 300
  );
  const listLatency = await measureCommand(
    host.state, "registry.list", { surface: "ai" }, 300
  );
  const describeLatency = await measureCommand(
    host.state,
    "registry.describe",
    { command: "project.register", version: 1 },
    300
  );

  const invalidTimes = [];
  for (let i = 0; i < 200; i++) {
    const started = performance.now();
    const invalid = await callCommand(
      host.state,
      "project.register",
      { root_uri: "file:///missing-name" }
    );
    if (invalid.error?.code !== "VALIDATION_FAILED") {
      throw new Error("invalid request did not fail validation");
    }
    invalidTimes.push(performance.now() - started);
  }

  const omittedVersion = await callCommand(
    host.state, "system.status", {}
  );
  const incompatibleVersion = await callCommand(
    host.state, "system.status", {}, 999
  );

  const aiList = await callCommand(
    host.state, "registry.list", { surface: "ai" }
  );
  const adapterList = await callCommand(
    host.state, "registry.list", { surface: "adapter" }
  );
  const dashboardList = await callCommand(
    host.state, "registry.list", { surface: "dashboard" }
  );
  const projectDescribe = await callCommand(
    host.state,
    "registry.describe",
    { command: "project.register", version: 1 }
  );

  const cliCommands = spawnRust(["commands"], host.env);
  if (cliCommands.status !== 0) {
    throw new Error("Rust commands CLI failed: " + cliCommands.stderr);
  }
  const cliCatalog = JSON.parse(cliCommands.stdout.trim());
  const cliHelp = spawnRust(["help"], host.env);
  if (cliHelp.status !== 0) {
    throw new Error("Rust help CLI failed: " + cliHelp.stderr);
  }
  const cliDescribe = spawnRust(
    ["describe", "project.register", "1"],
    host.env
  );
  if (cliDescribe.status !== 0) {
    throw new Error("Rust describe CLI failed: " + cliDescribe.stderr);
  }
  const cliDescription = JSON.parse(cliDescribe.stdout.trim());

  const legacyNodeClientStatus = await callHostWithState(
    host.state,
    "system.status",
    {},
    3000,
    "spike9-node-client"
  );

  const registryTests = spawnSync(
    "cargo",
    ["test", "registry::tests", "--", "--nocapture"],
    {
      cwd: rustRoot,
      windowsHide: true,
      encoding: "utf8",
      timeout: 30000
    }
  );
  if (registryTests.status !== 0) {
    throw new Error("registry tests failed: " + registryTests.stderr);
  }

  const dashboardIds = host.state.dashboard.commands;
  const dashboardRegistryIds = dashboardList.result.commands.map(
    item => item.id
  );
  const cliIds = cliCatalog.commands.map(item => item.id);
  const registryCliIds = registryJson.commands
    .filter(command => command.surfaces.includes("cli"))
    .map(command => command.id)
    .sort();

  const idle = await idleSample(host.child.pid);

  const spike8Build =
    spike8.build_and_dependency_economics.spike8_storage_build;
  const spike8Rust =
    spike8.measurements.rust.idle_after_restart;

  const contextBytes = {
    registry_pretty_bytes: byteLength(registrySource),
    registry_minified_bytes: byteLength(registryMinified),
    ai_compact_list_bytes: byteLength(aiList.result),
    adapter_compact_list_bytes: byteLength(adapterList.result),
    dashboard_compact_list_bytes: byteLength(dashboardList.result),
    project_register_description_bytes: byteLength(projectDescribe.result),
    cli_help_bytes: byteLength(cliHelp.stdout)
  };
  const heuristicTokens = Object.fromEntries(
    Object.entries(contextBytes).map(([key, bytes]) => [
      key.replace("_bytes", "_heuristic_tokens"),
      Math.ceil(bytes / 4)
    ])
  );

  const result = {
    benchmark: "phase1-spike9-command-registry-windows",
    recorded_at: new Date().toISOString(),
    decision_informed:
      "Which Phase 1 schema/IDL and command-registry shape should become RELAY's semantic machine-contract source of truth.",
    hypothesis:
      "A plain JSON registry using a bounded JSON Schema 2020-12 subset can drive Rust validation and derived CLI/dashboard/adapter/AI discovery without another runtime dependency or compiler while keeping compatibility rules explicit.",

    runtime: {
      os: {
        platform: process.platform,
        release: os.release(),
        arch: os.arch()
      },
      rustc: execFileSync(
        "rustc", ["--version"],
        { encoding: "utf8", windowsHide: true }
      ).trim(),
      cargo: execFileSync(
        "cargo", ["--version"],
        { encoding: "utf8", windowsHide: true }
      ).trim(),
      cpu_model: os.cpus()[0]?.model.trim() ?? "unknown",
      logical_cpu_count: os.cpus().length,
      total_memory_bytes: os.totalmem()
    },
    selected_contract: {
      format: "JSON command registry",
      registry_format: registryJson.registry_format,
      registry_id: registryJson.registry_id,
      schema_dialect: registryJson.schema_dialect,
      schema_profile:
        "bounded RELAY subset of JSON Schema 2020-12; unsupported keywords fail registry validation",
      command_count: registryJson.commands.length,
      reserved_command_ids: registryJson.reserved_command_ids,
      deprecated_command_ids: registryJson.deprecated_command_ids
    },

    measurements: {
      startup_to_state_ms: +startupMs.toFixed(3),
      status_with_registry_validation: statusLatency,
      registry_list_ai: listLatency,
      registry_describe_project_register: describeLatency,
      invalid_request_rejection: summarize(invalidTimes),
      idle_host_with_embedded_dashboard: idle,
      context_bytes: contextBytes,
      context_heuristic_tokens_at_4_bytes_per_token: heuristicTokens,
      context_ratios: {
        ai_list_vs_full_minified:
          +(contextBytes.ai_compact_list_bytes /
            contextBytes.registry_minified_bytes).toFixed(6),
        one_description_vs_full_minified:
          +(contextBytes.project_register_description_bytes /
            contextBytes.registry_minified_bytes).toFixed(6)
      }
    },
    build_economics: {
      spike8_storage_baseline: {
        direct_dependency_count: spike8Build.direct_dependency_count,
        resolved_package_count: spike8Build.resolved_package_count_including_root,
        release_binary_bytes: spike8Build.release_binary_bytes,
        idle_rss_bytes: spike8Rust.rss_bytes
      },
      spike9_registry_enabled: {
        clean_release_build_ms: +buildMs.toFixed(3),
        direct_dependency_count: directDependencyCount,
        resolved_package_count: metadata.packages.length,
        release_binary_bytes: binaryBytes,
        idle_rss_bytes: idle.rss_bytes
      },
      deltas: {
        direct_dependencies:
          directDependencyCount - spike8Build.direct_dependency_count,
        resolved_packages:
          metadata.packages.length - spike8Build.resolved_package_count_including_root,
        release_binary_bytes:
          binaryBytes - spike8Build.release_binary_bytes,
        idle_rss_bytes:
          idle.rss_bytes - spike8Rust.rss_bytes
      }
    },

    alternative_costs_and_tools: {
      full_rust_jsonschema_engine_experiment: {
        crate: "jsonschema 0.57.0",
        default_features: false,
        resolved_packages: 80,
        tiny_validator_release_exe_bytes: 4229120,
        first_optimized_build_ms_on_fixture: 72535.661,
        note:
          "Auxiliary isolated cost probe; not added to RELAY dependencies."
      },
      protobuf: {
        protoc_installed_on_fixture: Boolean(installedTool("protoc")),
        compatibility_strength:
          "strong field-number and reserved-tag evolution discipline",
        fit_cost:
          "requires code generation/binary schema mapping for primarily JSON CLI/dashboard/AI surfaces"
      },
      typespec: {
        tsp_installed_on_fixture: Boolean(installedTool("tsp")),
        fit:
          "can model APIs and emit JSON Schema",
        cost:
          "adds a compiler/emitter toolchain that RELAY does not otherwise need"
      },
      cue: {
        cue_installed_on_fixture: Boolean(installedTool("cue")),
        fit:
          "strong constraint and validation language",
        cost:
          "adds another language/toolchain for a small local JSON contract surface"
      }
    },
    derived_surfaces: {
      dashboard_state_command_ids: dashboardIds,
      dashboard_registry_command_ids: dashboardRegistryIds,
      cli_registry_command_ids: registryCliIds,
      cli_local_command_ids: cliIds,
      ai_command_count: aiList.result.commands.length,
      adapter_command_count: adapterList.result.commands.length,
      cli_help_bytes: byteLength(cliHelp.stdout),
      project_register_cli_matches_host:
        JSON.stringify(cliDescription) === JSON.stringify(projectDescribe.result)
    },

    compatibility_evidence: {
      omitted_command_version_resolved_to:
        omittedVersion.command_version,
      explicit_unsupported_version_error:
        incompatibleVersion.error?.code ?? null,
      legacy_node_client_ok:
        legacyNodeClientStatus.ok === true,
      legacy_node_client_resolved_command_version:
        legacyNodeClientStatus.command_version ?? null,
      registry_tests_exit_code: registryTests.status
    },
    pass_fail: {
      embedded_registry_has_13_commands:
        registryJson.commands.length === 13,
      registry_tests_pass:
        registryTests.status === 0,
      no_new_direct_runtime_dependencies:
        directDependencyCount === spike8Build.direct_dependency_count,
      no_new_resolved_runtime_packages:
        metadata.packages.length ===
          spike8Build.resolved_package_count_including_root,
      legacy_version_omitting_client_still_works:
        legacyNodeClientStatus.ok === true &&
        legacyNodeClientStatus.command_version === 1,
      incompatible_command_version_fails_closed:
        incompatibleVersion.ok === false &&
        incompatibleVersion.error?.code ===
          "COMMAND_VERSION_INCOMPATIBLE",
      invalid_arguments_fail_before_business_logic:
        (await callCommand(
          host.state,
          "project.register",
          { root_uri: "file:///missing-name" }
        )).error?.code === "VALIDATION_FAILED",

      dashboard_command_set_is_registry_derived:
        JSON.stringify(dashboardIds) ===
        JSON.stringify(dashboardRegistryIds),
      cli_command_set_is_registry_derived:
        JSON.stringify(cliIds) === JSON.stringify(registryCliIds),
      cli_description_matches_host_registry:
        JSON.stringify(cliDescription) ===
        JSON.stringify(projectDescribe.result),
      dashboard_does_not_expose_shutdown:
        !dashboardIds.includes("system.shutdown"),
      compact_ai_discovery_is_smaller_than_full_registry:
        contextBytes.ai_compact_list_bytes <
        contextBytes.registry_minified_bytes,
      one_command_description_is_smaller_than_full_registry:
        contextBytes.project_register_description_bytes <
        contextBytes.registry_minified_bytes,
      status_runtime_validation_p50_under_2ms:
        statusLatency.p50_ms < 2,
      deterministic_invalid_rejection_p50_under_2ms:
        summarize(invalidTimes).p50_ms < 2
    },
    limitations: [
      "The selected validator intentionally implements only the JSON Schema 2020-12 keywords RELAY registry format 1 allows. Unsupported keywords fail registry validation instead of being silently ignored.",
      "The registry contains the current 13-command Phase 1 contract surface; this selects the source-of-truth mechanism, not a frozen public command catalog.",
      "Command-version omission currently resolves to the latest registered version. Public clients that need reproducibility may pin a version; breaking schema changes require a new command contract version.",
      "Context token counts are a transparent 4 UTF-8 bytes/token heuristic because no tokenizer package is installed; they are not model-specific token counts.",
      "The full jsonschema crate cost probe was an isolated auxiliary executable and included first-build/download effects; its package and binary footprint are more decision-relevant than exact build time.",
      "TypeSpec, CUE, and protoc were not installed on the fixture, so they were assessed for contract/toolchain fit rather than implemented as full alternate RELAY prototypes.",
      "The registry is embedded at build time in this Phase 1 prototype. Signed/external contract packs and extension registries remain later design work.",
      "No adapter product behavior or UEFN commands were added in Spike 9."
    ]
  };

  if (!Object.values(result.pass_fail).every(Boolean)) {
    throw new Error(
      "Spike 9 gate failed: " +
      Object.entries(result.pass_fail)
        .filter(([, value]) => !value)
        .map(([key]) => key)
        .join(", ")
    );
  }

  await fs.mkdir(path.dirname(resultPath), { recursive: true });
  await fs.writeFile(
    resultPath,
    JSON.stringify(result, null, 2) + "\n",
    "utf8"
  );
  console.log(JSON.stringify(result, null, 2));
} finally {
  if (host?.child && host.child.exitCode === null) {
    await stopHost(host).catch(() => {
      host.child.kill("SIGKILL");
    });
  }
  await sleep(100);
  await fs.rm(tempRoot, { recursive: true, force: true });
}
