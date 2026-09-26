import assert from "node:assert/strict";
import test from "node:test";
import {
  chooseResourceMode,
  creatorProcesses,
  schedulingDecision
} from "../src/resource-scheduler.mjs";

test("Spike 5 detects creator workloads without burdening unrelated processes", () => {
  const names = [
    "explorer",
    "UnrealEditorFortnite-Win64-Shipping",
    "node",
    "Krita"
  ];
  assert.deepEqual(
    creatorProcesses(names),
    ["UnrealEditorFortnite-Win64-Shipping", "Krita"]
  );
  assert.equal(creatorProcesses(["explorer", "node"]).length, 0);
});

test("Spike 5 foreground-safe mode defers optional heavy work", () => {
  const state = chooseResourceMode({
    processNames: ["UnrealEditorFortnite-Win64-Shipping", "explorer"]
  });
  assert.equal(state.mode, "foreground_safe");

  assert.deepEqual(
    schedulingDecision({ kind: "local_ai", interruptible: true }, state),
    { action: "defer", reason: "foreground_creator_workload", mode: "foreground_safe" }
  );
  assert.deepEqual(
    schedulingDecision({ kind: "deep_index", interruptible: false }, state),
    { action: "do_not_start", reason: "foreground_creator_workload", mode: "foreground_safe" }
  );
  assert.deepEqual(
    schedulingDecision({ kind: "diagnostic_burst", interruptible: true }, state),
    { action: "run_bounded", reason: "explicit_diagnostic_work", mode: "foreground_safe" }
  );
});

test("Spike 5 balanced and idle-boost modes permit normal background work", () => {
  const balanced = chooseResourceMode({ processNames: ["explorer", "node"] });
  assert.equal(balanced.mode, "balanced");
  assert.equal(
    schedulingDecision({ kind: "storage_maintenance", interruptible: true }, balanced).action,
    "run"
  );

  const idle = chooseResourceMode({ processNames: [], explicitIdleBoost: true });
  assert.equal(idle.mode, "idle_boost");
  assert.equal(
    schedulingDecision({ kind: "deep_index", interruptible: true }, idle).action,
    "run"
  );
});
