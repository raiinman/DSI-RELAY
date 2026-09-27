import crypto from "node:crypto";
import fs from "node:fs";
import fsp from "node:fs/promises";
import path from "node:path";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const normalizeRelative = value => value.split(path.sep).join("/");

export async function scanTree(root, { hashContent = false } = {}) {
  const files = new Map();
  const inodeToPath = new Map();
  const stack = [root];
  let directories = 0;
  let statCalls = 0;
  let contentBytesRead = 0;

  while (stack.length) {
    const directory = stack.pop();
    directories += 1;
    const entries = await fsp.readdir(directory, { withFileTypes: true });
    for (const entry of entries) {
      const absolute = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        stack.push(absolute);
        continue;
      }
      if (!entry.isFile()) continue;

      const stat = await fsp.stat(absolute, { bigint: true });
      statCalls += 1;
      const relative = normalizeRelative(path.relative(root, absolute));
      const inode = stat.ino.toString(16);
      const record = {
        path: relative,
        inode,
        size: Number(stat.size),
        mtime_ns: stat.mtimeNs.toString()
      };
      if (hashContent) {
        const bytes = await fsp.readFile(absolute);
        contentBytesRead += bytes.length;
        record.sha256 = crypto.createHash("sha256").update(bytes).digest("hex");
      }
      files.set(relative, record);
      inodeToPath.set(inode, relative);
    }
  }

  return {
    root,
    files,
    inodeToPath,
    metrics: {
      files: files.size,
      directories,
      stat_calls: statCalls,
      logical_content_bytes_read: contentBytesRead
    }
  };
}

export function diffSnapshots(previous, current) {
  const added = [];
  const removed = [];
  const modified = [];
  const renamed = [];
  const previousByInode = previous.inodeToPath;
  const currentByInode = current.inodeToPath;

  for (const [relative, before] of previous.files) {
    const after = current.files.get(relative);
    if (!after) {
      const movedTo = currentByInode.get(before.inode);
      if (movedTo && movedTo !== relative) renamed.push({ from: relative, to: movedTo, inode: before.inode });
      else removed.push(relative);
      continue;
    }
    if (before.inode !== after.inode || before.size !== after.size || before.mtime_ns !== after.mtime_ns) {
      modified.push(relative);
    }
  }

  const renamedTargets = new Set(renamed.map(item => item.to));
  for (const [relative, after] of current.files) {
    if (previous.files.has(relative) || renamedTargets.has(relative)) continue;
    const oldPath = previousByInode.get(after.inode);
    if (!oldPath) added.push(relative);
  }

  const candidatePaths = new Set([...added, ...modified, ...renamed.map(item => item.to)]);
  return { added, removed, modified, renamed, candidatePaths };
}

export async function parsePaths(root, paths) {
  const parsed = [];
  let logicalBytesRead = 0;
  for (const relative of paths) {
    const absolute = path.join(root, ...relative.split("/"));
    try {
      const bytes = await fsp.readFile(absolute);
      logicalBytesRead += bytes.length;
      parsed.push({
        path: relative,
        bytes: bytes.length,
        sha256: crypto.createHash("sha256").update(bytes).digest("hex")
      });
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
  }
  return { parsed, logical_bytes_read: logicalBytesRead };
}

export function watchTree(root, onEvent, onError = () => {}) {
  const watcher = fs.watch(root, { recursive: true }, (eventType, filename) => {
    if (!filename) return;
    onEvent({
      event_type: eventType,
      path: normalizeRelative(String(filename)),
      observed_at_ms: performance.now(),
      observed_at_epoch_ms: Date.now()
    });
  });
  watcher.on("error", onError);
  return watcher;
}

export function parseUsnQuery(output) {
  const field = label => {
    const match = output.match(new RegExp(`^${label}\\s*:\\s*(0x[0-9a-fA-F]+)`, "m"));
    return match?.[1] ?? null;
  };
  return {
    journal_id: field("Usn Journal ID"),
    first_usn: field("First Usn"),
    next_usn: field("Next Usn"),
    lowest_valid_usn: field("Lowest Valid Usn"),
    max_usn: field("Max Usn")
  };
}

export function checkUsnContinuity(checkpoint, current) {
  if (!checkpoint || !current?.journal_id || !current?.first_usn || !current?.next_usn) {
    return { valid: false, reason: "missing_metadata" };
  }
  if (checkpoint.journal_id.toLowerCase() !== current.journal_id.toLowerCase()) {
    return { valid: false, reason: "journal_id_changed" };
  }
  const saved = BigInt(checkpoint.next_usn);
  const first = BigInt(current.first_usn);
  const next = BigInt(current.next_usn);
  if (saved < first) return { valid: false, reason: "checkpoint_trimmed" };
  if (saved > next) return { valid: false, reason: "checkpoint_ahead_of_journal" };
  return { valid: true, reason: "continuous" };
}

export async function queryUsnJournal(drive = "C:") {
  const start = performance.now();
  const { stdout } = await execFileAsync("fsutil.exe", ["usn", "queryjournal", drive], {
    windowsHide: true,
    encoding: "utf8"
  });
  return { ...parseUsnQuery(stdout), elapsed_ms: performance.now() - start };
}

export async function probeUsnRead(drive, startUsn) {
  const start = performance.now();
  try {
    const { stdout, stderr } = await execFileAsync(
      "fsutil.exe",
      ["usn", "readjournal", drive, `startusn=${startUsn}`, "csv"],
      { windowsHide: true, encoding: "utf8", maxBuffer: 4 * 1024 * 1024 }
    );
    return { available: true, elapsed_ms: performance.now() - start, stdout, stderr };
  } catch (error) {
    return {
      available: false,
      elapsed_ms: performance.now() - start,
      code: error.code ?? null,
      stdout: String(error.stdout ?? ""),
      stderr: String(error.stderr ?? error.message ?? "")
    };
  }
}
