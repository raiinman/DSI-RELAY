import fs from "node:fs/promises";
import { STATE_DIR, STATE_PATH } from "./config.mjs";

export async function writeState(state) {
  await fs.mkdir(STATE_DIR, { recursive: true });
  const temp = `${STATE_PATH}.${process.pid}.tmp`;
  await fs.writeFile(temp, JSON.stringify(state, null, 2), "utf8");
  await fs.rm(STATE_PATH, { force: true });
  await fs.rename(temp, STATE_PATH);
}

export async function readState() {
  return JSON.parse(await fs.readFile(STATE_PATH, "utf8"));
}

export async function clearState(expectedPid = process.pid) {
  try {
    const state = await readState();
    if (state.pid !== expectedPid) return;
    await fs.unlink(STATE_PATH);
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
}
