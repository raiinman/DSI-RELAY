const CREATOR_PROCESS_PATTERNS = Object.freeze([
  /UnrealEditorFortnite/i,
  /FortniteClient/i,
  /UnrealEditor/i,
  /Blender/i,
  /Krita/i
]);

export function creatorProcesses(processNames = []) {
  return processNames.filter(name => CREATOR_PROCESS_PATTERNS.some(pattern => pattern.test(name)));
}

export function chooseResourceMode({ processNames = [], explicitIdleBoost = false } = {}) {
  const creators = creatorProcesses(processNames);
  if (creators.length) {
    return {
      mode: "foreground_safe",
      reason: "creator_workload_active",
      creator_processes: creators
    };
  }
  if (explicitIdleBoost) {
    return {
      mode: "idle_boost",
      reason: "explicit_idle_boost",
      creator_processes: []
    };
  }
  return {
    mode: "balanced",
    reason: "no_creator_workload_detected",
    creator_processes: []
  };
}

export function schedulingDecision(job, resourceState) {
  const mode = resourceState.mode;
  const backgroundKinds = new Set([
    "deep_index",
    "storage_maintenance",
    "compatibility_scan",
    "local_ai",
    "inactive_project_reconciliation"
  ]);

  if (mode === "foreground_safe" && backgroundKinds.has(job.kind)) {
    return {
      action: job.interruptible === false ? "do_not_start" : "defer",
      reason: "foreground_creator_workload",
      mode
    };
  }

  if (mode === "foreground_safe" && job.kind === "diagnostic_burst") {
    return {
      action: "run_bounded",
      reason: "explicit_diagnostic_work",
      mode
    };
  }

  return {
    action: "run",
    reason: mode === "idle_boost" ? "idle_capacity_available" : "normal_policy",
    mode
  };
}
