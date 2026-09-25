import crypto from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { spawnSync } from "node:child_process";

function fail(code, message) {
  throw Object.assign(new Error(message), { code });
}

export async function sha256File(file) {
  const hash = crypto.createHash("sha256");
  hash.update(await fs.readFile(file));
  return hash.digest("hex");
}

function psLiteral(value) {
  return "'" + String(value).replaceAll("'", "''") + "'";
}

export function verifyDetachedCmsBundle({
  powershellExe,
  bundle,
  signature,
  expectedPublisher
}) {
  const script =
    "Add-Type -AssemblyName System.Security;" +
    "$sha=[Security.Cryptography.SHA256]::Create();" +
    "try{$digest=$sha.ComputeHash([IO.File]::ReadAllBytes(" +
      psLiteral(bundle) + "))}finally{$sha.Dispose()};" +
    "$cms=[Security.Cryptography.Pkcs.SignedCms]::new();" +
    "$cms.Decode([IO.File]::ReadAllBytes(" + psLiteral(signature) + "));" +
    "$cms.CheckSignature($false);" +
    "$signed=$cms.ContentInfo.Content;" +
    "if(-not [Collections.StructuralComparisons]::StructuralEqualityComparer.Equals($signed,$digest)){throw 'bundle digest mismatch'};" +
    "$subject=$cms.SignerInfos[0].Certificate.Subject;" +
    "if($subject -ne " + psLiteral(expectedPublisher) + "){throw 'publisher mismatch'};" +
    "[pscustomobject]@{subject=$subject;signers=$cms.SignerInfos.Count;digest_bytes=$signed.Length}|ConvertTo-Json -Compress";
  const result = spawnSync(
    powershellExe,
    ["-NoProfile", "-Command", script],
    { encoding: "utf8", windowsHide: true }
  );
  if (result.status !== 0) {
    fail(
      "UPDATE_SIGNATURE_INVALID",
      (result.stderr || result.stdout || "CMS bundle digest verification failed").trim()
    );
  }
  return JSON.parse(result.stdout.trim());
}

export function extractZip(tarExe, bundle, destination) {
  const result = spawnSync(
    tarExe,
    ["-xf", bundle, "-C", destination],
    { encoding: "utf8", windowsHide: true }
  );
  if (result.status !== 0) {
    fail("UPDATE_EXTRACT_FAILED", result.stderr || result.stdout);
  }
}


export async function readReleaseManifest(directory) {
  const file = path.join(directory, "release.json");
  let manifest;
  try {
    manifest = JSON.parse(await fs.readFile(file, "utf8"));
  } catch (error) {
    fail("UPDATE_MANIFEST_INVALID", "read release manifest: " + error.message);
  }
  if (
    !manifest ||
    typeof manifest.version !== "string" ||
    !manifest.version ||
    typeof manifest.channel !== "string" ||
    typeof manifest.source !== "string" ||
    !Number.isInteger(manifest.max_storage_schema) ||
    !Array.isArray(manifest.components)
  ) {
    fail("UPDATE_MANIFEST_INVALID", "release manifest fields are invalid");
  }
  return manifest;
}

export async function verifyReleaseComponents(directory, manifest) {
  for (const component of manifest.components) {
    if (
      !component ||
      typeof component.path !== "string" ||
      typeof component.sha256 !== "string" ||
      !/^[0-9a-f]{64}$/i.test(component.sha256)
    ) {
      fail("UPDATE_MANIFEST_INVALID", "component entry is invalid");
    }
    const absolute = path.resolve(directory, ...component.path.split("/"));
    const root = path.resolve(directory) + path.sep;
    if (!absolute.startsWith(root)) {
      fail("UPDATE_MANIFEST_INVALID", "component escapes update directory");
    }
    const actual = await sha256File(absolute).catch(error => {
      fail("UPDATE_COMPONENT_MISSING", error.message);
    });
    if (actual.toLowerCase() !== component.sha256.toLowerCase()) {
      fail(
        "UPDATE_COMPONENT_HASH_MISMATCH",
        "component hash mismatch: " + component.path
      );
    }
  }
  return true;
}


export async function stageSignedBundle({
  powershellExe,
  tarExe,
  bundle,
  signature,
  expectedPublisher,
  installRoot
}) {
  verifyDetachedCmsBundle({
    powershellExe,
    bundle,
    signature,
    expectedPublisher
  });

  const stagingRoot = path.join(installRoot, "staging");
  const token = crypto.randomBytes(8).toString("hex");
  const staging = path.join(stagingRoot, token);
  await fs.mkdir(staging, { recursive: true });

  try {
    extractZip(tarExe, bundle, staging);
    let manifest = await readReleaseManifest(staging);
    await verifyReleaseComponents(staging, manifest);

    const versionsRoot = path.join(installRoot, "versions");
    const finalDir = path.join(versionsRoot, manifest.version);
    await fs.mkdir(versionsRoot, { recursive: true });

    if (await fs.stat(finalDir).then(() => true).catch(() => false)) {
      const existing = await readReleaseManifest(finalDir);
      await verifyReleaseComponents(finalDir, existing);
      if (
        existing.version !== manifest.version ||
        JSON.stringify(existing.components) !== JSON.stringify(manifest.components)
      ) {
        fail(
          "UPDATE_VERSION_CONFLICT",
          "existing version directory does not match signed update manifest"
        );
      }
      manifest = existing;
      await fs.rm(staging, { recursive: true, force: true });
    } else {
      await fs.rename(staging, finalDir);
    }

    return {
      manifest,
      version_dir: finalDir,
      bundle_sha256: await sha256File(bundle)
    };
  } catch (error) {
    await fs.rm(staging, { recursive: true, force: true }).catch(() => {});
    throw error;
  }
}

export async function readActiveVersion(installRoot) {
  const file = path.join(installRoot, "current.json");
  try {
    return JSON.parse(await fs.readFile(file, "utf8"));
  } catch {
    return null;
  }
}


export function assertStorageCompatibility(
  manifest,
  storageSchemaVersion
) {
  if (storageSchemaVersion > manifest.max_storage_schema) {
    fail(
      "UPDATE_STORAGE_SCHEMA_INCOMPATIBLE",
      "active storage schema is newer than this binary supports"
    );
  }
  return true;
}

export async function activateVersion({
  installRoot,
  staged,
  storageSchemaVersion
}) {
  assertStorageCompatibility(staged.manifest, storageSchemaVersion);

  const current = {
    version: staged.manifest.version,
    version_dir: staged.version_dir,
    bundle_sha256: staged.bundle_sha256,
    channel: staged.manifest.channel,
    source: staged.manifest.source,
    max_storage_schema: staged.manifest.max_storage_schema,
    components: staged.manifest.components,
    activated_at: new Date().toISOString()
  };

  await fs.mkdir(installRoot, { recursive: true });
  const temp = path.join(
    installRoot,
    "current." + crypto.randomBytes(8).toString("hex") + ".tmp"
  );
  const data = JSON.stringify(current, null, 2) + "\n";
  await fs.writeFile(temp, data, "utf8");
  const handle = await fs.open(temp, "r+");
  try {
    await handle.sync();
  } finally {
    await handle.close();
  }
  await fs.rename(temp, path.join(installRoot, "current.json"));
  return current;
}

export async function installSignedBundle(options) {
  const staged = await stageSignedBundle(options);
  const active = await activateVersion({
    installRoot: options.installRoot,
    staged,
    storageSchemaVersion: options.storageSchemaVersion
  });
  return { staged, active };
}

export async function uninstallBinaries(installRoot) {
  await fs.rm(installRoot, { recursive: true, force: true });
}
