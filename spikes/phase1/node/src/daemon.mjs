#!/usr/bin/env node
if (process.argv.includes("--stop")) {
  const { callHost } = await import("./client.mjs");
  try {
    const response = await callHost("system.shutdown");
    console.log(response.ok ? "relayd stopping" : JSON.stringify(response));
    process.exitCode = response.ok ? 0 : 3;
  } catch (error) {
    console.error(`relayd stop failed: ${error.message}`);
    process.exitCode = 4;
  }
} else {
  await import("./host.mjs");
}
