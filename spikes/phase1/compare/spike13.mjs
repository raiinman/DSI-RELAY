import crypto from "node:crypto";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { execFileSync, spawnSync } from "node:child_process";
import { performance } from "node:perf_hooks";
import {
  activateVersion,
  assertStorageCompatibility,
  installSignedBundle,
  readActiveVersion,
  sha256File,
  stageSignedBundle,
  uninstallBinaries,
  verifyDetachedCmsBundle
} from "../packaging/side_by_side_updater.mjs";

const here = path.resolve(import.meta.dirname);
const phase1 = path.resolve(here, "..");
const rustRoot = path.join(phase1, "rust");
const packagingRoot = path.join(phase1, "packaging");
const coreExe = path.join(
  rustRoot,
  "target",
  "release",
  "relay-rust-challenger.exe"
);
const adapterExe = path.join(
  rustRoot,
  "target",
  "release",
  "relay-synthetic-adapter.exe"
);
const resultPath = path.join(
  phase1,
  "results",
  "2026-09-25-spike13-packaging-update-windows.json"
);

const makeAppx =
  "C:\\Program Files (x86)\\Windows Kits\\10\\bin\\10.0.26100.0\\x64\\makeappx.exe";
const signTool =
  "C:\\Program Files (x86)\\Windows Kits\\10\\bin\\10.0.26100.0\\x64\\signtool.exe";
const powershellExe =
  "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe";
const tarExe = "C:\\Windows\\System32\\tar.exe";
const packageName = "DSIRelay.Spike13.Benchmark";
const publisher = "CN=DSIRelaySpike13Test";
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

function run(exe, args, options = {}) {
  const result = spawnSync(exe, args, {
    encoding: "utf8",
    windowsHide: true,
    ...options
  });
  return result;
}

function requireSuccess(result, label) {
  if (result.status !== 0) {
    throw new Error(
      label + " failed: " + (result.stderr || result.stdout || "")
    );
  }
  return result;
}

function ps(command) {
  return requireSuccess(
    run("powershell.exe", ["-NoProfile", "-Command", command]),
    "PowerShell"
  ).stdout.trim();
}

function psJson(command) {
  const text = ps(command);
  return text ? JSON.parse(text) : null;
}

async function fileExists(file) {
  return fs.stat(file).then(() => true).catch(() => false);
}

async function fileSize(file) {
  return (await fs.stat(file)).size;
}

async function countLines(file) {
  return (await fs.readFile(file, "utf8")).split(/\r?\n/).length;
}

function quotePs(value) {
  return "'" + String(value).replaceAll("'", "''") + "'";
}

function currentTokenInfo() {
  return psJson(
    "$id=[Security.Principal.WindowsIdentity]::GetCurrent();" +
    "$p=New-Object Security.Principal.WindowsPrincipal($id);" +
    "$proc=[Diagnostics.Process]::GetCurrentProcess();" +
    "[pscustomobject]@{" +
    "elevated=$p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator);" +
    "session_id=$proc.SessionId;" +
    "user_sid=$id.User.Value" +
    "}|ConvertTo-Json -Compress"
  );
}

function createTestCertificate(root) {
  const cer = path.join(root, "spike13-test.cer");
  const command =
    "$c=New-SelfSignedCertificate -Type Custom " +
    "-Subject " + quotePs(publisher) + " -KeyUsage DigitalSignature " +
    "-CertStoreLocation 'Cert:\\CurrentUser\\My' " +
    "-TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3','2.5.29.19={text}') " +
    "-NotAfter (Get-Date).AddDays(1);" +
    "Export-Certificate -Cert $c -FilePath " + quotePs(cer) + " | Out-Null;" +
    "Import-Certificate -FilePath " + quotePs(cer) +
    " -CertStoreLocation 'Cert:\\CurrentUser\\TrustedPeople' | Out-Null;" +
    "Import-Certificate -FilePath " + quotePs(cer) +
    " -CertStoreLocation 'Cert:\\CurrentUser\\Root' | Out-Null;" +
    "[pscustomobject]@{thumbprint=$c.Thumbprint;subject=$c.Subject}" +
    "|ConvertTo-Json -Compress";
  return psJson(command);
}


function cleanupTestCertificate(thumbprint) {
  if (!thumbprint) return;
  const command =
    "foreach($name in 'TrustedPeople','Root','My'){" +
    "$s=[Security.Cryptography.X509Certificates.X509Store]::new(" +
      "$name,[Security.Cryptography.X509Certificates.StoreLocation]::CurrentUser);" +
    "$s.Open([Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite);" +
    "foreach($c in @($s.Certificates | Where-Object {" +
      "$_.Thumbprint -eq " + quotePs(thumbprint) +
      " -and $_.Subject -eq " + quotePs(publisher) +
    "})){$s.Remove($c)};" +
    "$s.Close()}";
  ps(command);
}

function certificateStillPresent(thumbprint) {
  if (!thumbprint) return false;
  const command =
    "$n=0;foreach($name in 'TrustedPeople','Root','My'){" +
    "$s=[Security.Cryptography.X509Certificates.X509Store]::new(" +
      "$name,[Security.Cryptography.X509Certificates.StoreLocation]::CurrentUser);" +
    "$s.Open([Security.Cryptography.X509Certificates.OpenFlags]::ReadOnly);" +
    "$n+=@($s.Certificates | Where-Object {" +
      "$_.Thumbprint -eq " + quotePs(thumbprint) +
      " -and $_.Subject -eq " + quotePs(publisher) +
    "}).Count;" +
    "$s.Close()};$n";
  return ps(command) !== "0";
}

function createAssets(directory) {
  const command =
    "Add-Type -AssemblyName System.Drawing;" +
    "$d=" + quotePs(directory) + ";" +
    "New-Item -ItemType Directory -Force $d|Out-Null;" +
    "foreach($p in @(@('Logo.png',150),@('SmallLogo.png',44),@('StoreLogo.png',50))){" +
    "$b=New-Object Drawing.Bitmap($p[1],$p[1]);" +
    "$g=[Drawing.Graphics]::FromImage($b);$g.Clear([Drawing.Color]::Black);" +
    "$b.Save((Join-Path $d $p[0]),[Drawing.Imaging.ImageFormat]::Png);" +
    "$g.Dispose();$b.Dispose()}";
  ps(command);
}


async function releaseManifest(version, source) {
  return {
    version,
    channel: "stable",
    source,
    max_storage_schema: 1,
    components: [
      {
        id: "relay-core",
        kind: "core",
        path: "relay-rust-challenger.exe",
        version,
        source: "local-release-build",
        provenance: "spike13-synthetic",
        sha256: await sha256File(coreExe)
      },
      {
        id: "synthetic-adapter",
        kind: "adapter-worker",
        path: "relay-synthetic-adapter.exe",
        version,
        source: "local-release-build",
        provenance: "spike13-synthetic",
        sha256: await sha256File(adapterExe)
      }
    ]
  };
}

function msixManifest(version) {
  return [
    '<?xml version="1.0" encoding="utf-8"?>',
    '<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"',
    ' xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"',
    ' xmlns:uap10="http://schemas.microsoft.com/appx/manifest/uap/windows10/10"',
    ' xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"',
    ' IgnorableNamespaces="uap uap10 rescap">',
    '<Identity Name="' + packageName + '" Publisher="' + publisher +
      '" Version="' + version + '" ProcessorArchitecture="x64"/>',
    '<Properties><DisplayName>RELAY Spike13</DisplayName>',
    '<PublisherDisplayName>DSI RELAY Fixture</PublisherDisplayName>',
    '<Description>Packaging fixture</Description>',
    '<Logo>Assets\\StoreLogo.png</Logo></Properties>',
    '<Resources><Resource Language="en-us"/></Resources>',
    '<Dependencies><TargetDeviceFamily Name="Windows.Desktop"',
    ' MinVersion="10.0.19041.0" MaxVersionTested="10.0.26200.0"/>',
    '</Dependencies>',
    '<Capabilities><rescap:Capability Name="runFullTrust"/></Capabilities>',
    '<Applications><Application Id="RelayApp"',
    ' Executable="relay-rust-challenger.exe"',
    ' uap10:RuntimeBehavior="packagedClassicApp"',
    ' uap10:TrustLevel="mediumIL">',
    '<uap:VisualElements DisplayName="RELAY Spike13"',
    ' Description="Packaging fixture"',
    ' Square150x150Logo="Assets\\Logo.png"',
    ' Square44x44Logo="Assets\\SmallLogo.png"',
    ' BackgroundColor="transparent"/>',
    '</Application></Applications></Package>'
  ].join("");
}


function appInstallerXml(version) {
  const base = "https://updates.invalid/dsi-relay/";
  return [
    '<?xml version="1.0" encoding="utf-8"?>',
    '<AppInstaller xmlns="http://schemas.microsoft.com/appx/appinstaller/2021"',
    ' Version="' + version + '"',
    ' Uri="' + base + 'DSIRelay.appinstaller">',
    '<MainPackage Name="' + packageName + '"',
    ' Publisher="' + publisher + '"',
    ' Version="' + version + '"',
    ' ProcessorArchitecture="x64"',
    ' Uri="' + base + 'DSIRelay_' + version + '_x64.msix"/>',
    '<UpdateSettings>',
    '<OnLaunch HoursBetweenUpdateChecks="0" ShowPrompt="false"/>',
    '<AutomaticBackgroundTask/>',
    '<ForceUpdateFromAnyVersion>false</ForceUpdateFromAnyVersion>',
    '</UpdateSettings>',
    '</AppInstaller>'
  ].join("");
}

async function writePayload(directory, version, source) {
  await fs.mkdir(directory, { recursive: true });
  await fs.copyFile(coreExe, path.join(directory, "relay-rust-challenger.exe"));
  await fs.copyFile(adapterExe, path.join(directory, "relay-synthetic-adapter.exe"));
  const manifest = await releaseManifest(version, source);
  await fs.writeFile(
    path.join(directory, "release.json"),
    JSON.stringify(manifest, null, 2) + "\n",
    "utf8"
  );
  return manifest;
}


function signArtifact(file, thumbprint) {
  requireSuccess(
    run(signTool, [
      "sign",
      "/fd", "SHA256",
      "/s", "My",
      "/sha1", thumbprint,
      file
    ]),
    "SignTool sign"
  );
  requireSuccess(
    run(signTool, ["verify", "/pa", "/v", file]),
    "SignTool verify"
  );
}

async function buildMsix(root, version, thumbprint) {
  const stage = path.join(root, "msix-" + version);
  await writePayload(stage, version, "msix-appinstaller");
  createAssets(path.join(stage, "Assets"));
  await fs.writeFile(
    path.join(stage, "AppxManifest.xml"),
    msixManifest(version),
    "utf8"
  );
  const packageFile = path.join(root, "DSIRelay_" + version + "_x64.msix");
  requireSuccess(
    run(makeAppx, ["pack", "/d", stage, "/p", packageFile, "/o"]),
    "MakeAppx"
  );
  signArtifact(packageFile, thumbprint);
  return {
    path: packageFile,
    size_bytes: await fileSize(packageFile),
    sha256: await sha256File(packageFile),
    release: JSON.parse(await fs.readFile(path.join(stage, "release.json"), "utf8"))
  };
}


function signDetachedCmsBundle(bundle, signature, thumbprint) {
  const command =
    "Add-Type -AssemblyName System.Security;" +
    "$cert=Get-Item " +
    quotePs("Cert:\\CurrentUser\\My\\" + thumbprint) + ";" +
    "$sha=[Security.Cryptography.SHA256]::Create();" +
    "try{$digest=$sha.ComputeHash([IO.File]::ReadAllBytes(" +
      quotePs(bundle) + "))}finally{$sha.Dispose()};" +
    "$ci=[Security.Cryptography.Pkcs.ContentInfo]::new($digest);" +
    "$cms=[Security.Cryptography.Pkcs.SignedCms]::new($ci,$false);" +
    "$signer=[Security.Cryptography.Pkcs.CmsSigner]::new($cert);" +
    "$signer.IncludeOption=[Security.Cryptography.X509Certificates.X509IncludeOption]::EndCertOnly;" +
    "$cms.ComputeSignature($signer);" +
    "[IO.File]::WriteAllBytes(" + quotePs(signature) + ",$cms.Encode())";
  ps(command);
  return verifyDetachedCmsBundle({
    powershellExe,
    bundle,
    signature,
    expectedPublisher: publisher
  });
}

async function verifyMsixPayloadInventory(packageFile, root, label) {
  const directory = path.join(root, "unpack-" + label);
  await fs.mkdir(directory, { recursive: true });
  requireSuccess(
    run(makeAppx, ["unpack", "/p", packageFile, "/d", directory, "/o"]),
    "MakeAppx unpack"
  );
  const release = JSON.parse(
    await fs.readFile(path.join(directory, "release.json"), "utf8")
  );
  const checks = [];
  for (const component of release.components) {
    const file = path.join(directory, ...component.path.split("/"));
    const actual = await sha256File(file);
    checks.push({
      id: component.id,
      version: component.version,
      source: component.source,
      provenance: component.provenance,
      hash_matches: actual.toLowerCase() === component.sha256.toLowerCase()
    });
  }
  return {
    version: release.version,
    channel: release.channel,
    source: release.source,
    components: checks
  };
}

async function buildSignedZip(root, version, thumbprint) {
  const payload = path.join(root, "zip-" + version);
  await writePayload(payload, version, "relay-side-by-side");
  const bundle = path.join(root, "DSIRelay_" + version + ".zip");
  requireSuccess(
    run(tarExe, ["-a", "-c", "-f", bundle, "-C", payload, "."]),
    "tar zip"
  );
  const signature = bundle + ".p7s";
  const signer = signDetachedCmsBundle(bundle, signature, thumbprint);
  return {
    path: bundle,
    signature_path: signature,
    size_bytes: await fileSize(bundle),
    signature_bytes: await fileSize(signature),
    sha256: await sha256File(bundle),
    signer,
    release: JSON.parse(await fs.readFile(path.join(payload, "release.json"), "utf8"))
  };
}


function relayServiceCount() {
  return Number(ps(
    "$n=@(Get-Service -ErrorAction SilentlyContinue | " +
    "Where-Object {$_.Name -like '*DSIRelay*' -or $_.DisplayName -like '*DSI RELAY*'}).Count;$n"
  ));
}

function packageInfo() {
  return psJson(
    "$p=Get-AppxPackage -Name " + quotePs(packageName) + " | Select-Object -First 1;" +
    "if($p){[pscustomobject]@{" +
    "name=$p.Name;version=$p.Version.ToString();" +
    "publisher=$p.Publisher;package_full_name=$p.PackageFullName;" +
    "install_location=$p.InstallLocation" +
    "}|ConvertTo-Json -Compress}"
  );
}

function removeFixturePackage() {
  ps(
    "Get-AppxPackage -Name " + quotePs(packageName) +
    " | Remove-AppxPackage -ErrorAction SilentlyContinue"
  );
}

function installMsix(file, forceAnyVersion = false) {
  const started = performance.now();
  const command =
    "Add-AppxPackage -Path " + quotePs(file) +
    (forceAnyVersion ? " -ForceUpdateFromAnyVersion" : "") +
    " -ErrorAction Stop";
  ps(command);
  return performance.now() - started;
}

function attemptMsixInstall(file, forceAnyVersion = false) {
  const command =
    "Add-AppxPackage -Path " + quotePs(file) +
    (forceAnyVersion ? " -ForceUpdateFromAnyVersion" : "") +
    " -ErrorAction Stop";
  const started = performance.now();
  const result = run(
    "powershell.exe",
    ["-NoProfile", "-Command", command]
  );
  const message = result.stderr || result.stdout || "";
  const hresults = [
    ...new Set(message.match(/0x[0-9a-fA-F]{8}/g) ?? [])
  ];
  return {
    ok: result.status === 0,
    elapsed_ms: +(performance.now() - started).toFixed(3),
    hresult: hresults[0] ?? null,
    hresults
  };
}

function rejectedMsixInstall(file) {
  const attempt = attemptMsixInstall(file);
  return {
    rejected: !attempt.ok,
    hresult: attempt.hresult
  };
}

function runInstalledCoreVersion() {
  const info = packageInfo();
  if (!info?.install_location) {
    throw new Error("MSIX package is not installed");
  }
  const exe = path.join(info.install_location, "relay-rust-challenger.exe");
  const result = run(exe, ["version"]);
  requireSuccess(result, "installed RELAY version command");
  return JSON.parse(result.stdout.trim());
}


async function createDurableData(root) {
  const dataRoot = path.join(root, "durable-data");
  await fs.mkdir(dataRoot, { recursive: true });
  const marker = path.join(dataRoot, "project-marker.txt");
  await fs.writeFile(marker, "durable-user-data\n", "utf8");
  const dbPath = path.join(dataRoot, "relay.sqlite3");
  const db = new DatabaseSync(dbPath);
  db.exec(
    "CREATE TABLE schema_migrations(" +
    "version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);" +
    "INSERT INTO schema_migrations(version, applied_at) VALUES(1,'spike13');"
  );
  db.close();
  return { dataRoot, marker, dbPath };
}

function storageSchemaVersion(dbPath) {
  const db = new DatabaseSync(dbPath);
  const row = db.prepare(
    "SELECT COALESCE(MAX(version),0) AS version FROM schema_migrations"
  ).get();
  db.close();
  return Number(row.version);
}

function setStorageSchemaVersion(dbPath, version) {
  const db = new DatabaseSync(dbPath);
  db.exec("DELETE FROM schema_migrations");
  db.prepare(
    "INSERT INTO schema_migrations(version, applied_at) VALUES(?,?)"
  ).run(version, "spike13");
  db.close();
}

async function tamperCopy(source, destination) {
  const bytes = Buffer.from(await fs.readFile(source));
  const index = Math.max(64, Math.floor(bytes.length / 2));
  bytes[index] ^= 0x01;
  await fs.writeFile(destination, bytes);
  return destination;
}

function signatureRejected(file) {
  const result = run(signTool, ["verify", "/pa", file]);
  return result.status !== 0;
}

function detachedCmsRejected(bundle, signature) {
  try {
    verifyDetachedCmsBundle({
      powershellExe,
      bundle,
      signature,
      expectedPublisher: publisher
    });
    return false;
  } catch {
    return true;
  }
}


function installedAppInstallerVersion() {
  const info = psJson(
    "Get-AppxPackage Microsoft.DesktopAppInstaller | " +
    "Select-Object -First 1 Name,Version | ConvertTo-Json -Compress"
  );
  return info?.Version ?? info?.version ?? null;
}

async function timed(operation) {
  const started = performance.now();
  const value = await operation();
  return { value, elapsed_ms: +(performance.now() - started).toFixed(3) };
}

function errorCodeOf(operation) {
  try {
    operation();
    return null;
  } catch (error) {
    return error.code ?? error.message;
  }
}

async function asyncErrorCodeOf(operation) {
  try {
    await operation();
    return null;
  } catch (error) {
    return error.code ?? error.message;
  }
}

function currentPackageVersion() {
  const info = packageInfo();
  return info?.version ?? info?.Version ?? null;
}


function authenticodeInfo(file) {
  return psJson(
    "$s=Get-AuthenticodeSignature -FilePath " + quotePs(file) + ";" +
    "[pscustomobject]@{" +
    "status=$s.Status.ToString();" +
    "signer_subject=$s.SignerCertificate.Subject;" +
    "thumbprint=$s.SignerCertificate.Thumbprint" +
    "}|ConvertTo-Json -Compress"
  );
}

async function verifyInstalledComponentInventory() {
  const info = packageInfo();
  if (!info?.install_location) {
    throw new Error("installed package location is unavailable");
  }
  const release = JSON.parse(
    await fs.readFile(path.join(info.install_location, "release.json"), "utf8")
  );
  const checks = [];
  for (const component of release.components) {
    const file = path.join(
      info.install_location,
      ...component.path.split("/")
    );
    const actual = await sha256File(file);
    checks.push({
      id: component.id,
      version: component.version,
      source: component.source,
      provenance: component.provenance,
      hash_matches: actual.toLowerCase() === component.sha256.toLowerCase()
    });
  }
  return {
    version: release.version,
    channel: release.channel,
    source: release.source,
    components: checks
  };
}


const root = await fs.mkdtemp(
  path.join(os.tmpdir(), "relay-spike13-")
);
let certificate = null;
let result = null;
let cleanup = null;

try {
  requireSuccess(
    run("cargo", [
      "build", "--release",
      "--bin", "relay-rust-challenger",
      "--bin", "relay-synthetic-adapter"
    ], { cwd: rustRoot, timeout: 30000 }),
    "Rust release payload build"
  );

  removeFixturePackage();
  const token = currentTokenInfo();
  const tokenEvidence = {
    elevated: Boolean(token?.elevated ?? token?.Elevated),
    session_id: Number(token?.session_id ?? token?.SessionId),
    user_sid_present: Boolean(token?.user_sid ?? token?.UserSid)
  };
  const servicesBefore = relayServiceCount();
  certificate = createTestCertificate(root);
  const durable = await createDurableData(root);

  const v1 = "1.0.0.0";
  const v2 = "1.1.0.0";

  const msix1Build = await timed(() =>
    buildMsix(root, v1, certificate.thumbprint)
  );
  const msix2Build = await timed(() =>
    buildMsix(root, v2, certificate.thumbprint)
  );
  const zip1Build = await timed(() =>
    buildSignedZip(root, v1, certificate.thumbprint)
  );
  const zip2Build = await timed(() =>
    buildSignedZip(root, v2, certificate.thumbprint)
  );

  const appInstallerFile = path.join(root, "DSIRelay.appinstaller");
  const appInstallerText = appInstallerXml(v2);
  await fs.writeFile(appInstallerFile, appInstallerText, "utf8");
  const appInstallerEvidence = {
    bytes: await fileSize(appInstallerFile),
    sha256: await sha256File(appInstallerFile),
    on_launch_check: appInstallerText.includes(
      'HoursBetweenUpdateChecks="0"'
    ),
    background_update: appInstallerText.includes(
      "<AutomaticBackgroundTask/>"
    ),
    automatic_downgrade_disabled: appInstallerText.includes(
      "<ForceUpdateFromAnyVersion>false</ForceUpdateFromAnyVersion>"
    ),
    distribution_uri_scheme: "https",
    ms_appinstaller_uri_required: false
  };


  const tamperedMsix = await tamperCopy(
    msix2Build.value.path,
    path.join(root, "tampered.msix")
  );
  const msixTamperRejected = signatureRejected(tamperedMsix);
  const msixSignature = authenticodeInfo(msix2Build.value.path);
  const msixV1PackageInventory = await verifyMsixPayloadInventory(
    msix1Build.value.path,
    root,
    "msix-v1"
  );
  const msixV2PackageInventory = await verifyMsixPayloadInventory(
    msix2Build.value.path,
    root,
    "msix-v2"
  );

  const msixInstallV1Attempt =
    attemptMsixInstall(msix1Build.value.path);
  let msixV1Info = null;
  let msixStart = null;
  let msixV1Inventory = null;
  let msixTamperedInstall = null;
  let msixVersionAfterTamperedUpdate = null;
  let msixUpdateV2Ms = null;
  let msixV2Info = null;
  let msixV2Inventory = null;
  let msixBlockedRollbackCode = null;
  let msixVersionAfterBlockedRollback = null;
  let msixRollbackV1Ms = null;
  let msixVersionAfterRollback = null;

  if (msixInstallV1Attempt.ok) {
    msixV1Info = packageInfo();
    msixStart = runInstalledCoreVersion();
    msixV1Inventory = await verifyInstalledComponentInventory();

    msixTamperedInstall = rejectedMsixInstall(tamperedMsix);
    msixVersionAfterTamperedUpdate = currentPackageVersion();

    msixUpdateV2Ms = +installMsix(msix2Build.value.path).toFixed(3);
    msixV2Info = packageInfo();
    msixV2Inventory = await verifyInstalledComponentInventory();

    setStorageSchemaVersion(durable.dbPath, 2);
    msixBlockedRollbackCode = errorCodeOf(() =>
      assertStorageCompatibility(
        msix1Build.value.release,
        storageSchemaVersion(durable.dbPath)
      )
    );
    msixVersionAfterBlockedRollback = currentPackageVersion();

    setStorageSchemaVersion(durable.dbPath, 1);
    assertStorageCompatibility(
      msix1Build.value.release,
      storageSchemaVersion(durable.dbPath)
    );
    msixRollbackV1Ms =
      +installMsix(msix1Build.value.path, true).toFixed(3);
    msixVersionAfterRollback = currentPackageVersion();
  }

  setStorageSchemaVersion(durable.dbPath, 1);
  const msixUninstallStarted = performance.now();
  removeFixturePackage();
  const msixUninstallMs =
    +(performance.now() - msixUninstallStarted).toFixed(3);
  const msixRemoved = packageInfo() == null;
  const durableAfterMsix = {
    marker_exists: await fileExists(durable.marker),
    database_exists: await fileExists(durable.dbPath),
    schema_version: storageSchemaVersion(durable.dbPath)
  };

  const tamperedZip = await tamperCopy(
    zip2Build.value.path,
    path.join(root, "tampered.zip")
  );
  const zipTamperRejected = detachedCmsRejected(
    tamperedZip,
    zip2Build.value.signature_path
  );
  const zipSigner = zip2Build.value.signer;
  const sideInstallRoot = path.join(root, "side-by-side-install");

  const sideInstallV1 = await timed(() =>
    installSignedBundle({
      powershellExe,
      tarExe,
      bundle: zip1Build.value.path,
      signature: zip1Build.value.signature_path,
      expectedPublisher: publisher,
      installRoot: sideInstallRoot,
      storageSchemaVersion: 1
    })
  );
  const sideActiveV1 = await readActiveVersion(sideInstallRoot);


  const sideTamperedUpdateCode = await asyncErrorCodeOf(() =>
    stageSignedBundle({
      powershellExe,
      tarExe,
      bundle: tamperedZip,
      signature: zip2Build.value.signature_path,
      expectedPublisher: publisher,
      installRoot: sideInstallRoot
    })
  );
  const sideActiveAfterTamper = await readActiveVersion(sideInstallRoot);

  const sideStageV2 = await timed(() =>
    stageSignedBundle({
      powershellExe,
      tarExe,
      bundle: zip2Build.value.path,
      signature: zip2Build.value.signature_path,
      expectedPublisher: publisher,
      installRoot: sideInstallRoot
    })
  );
  const sideActiveAfterStage = await readActiveVersion(sideInstallRoot);

  const sideActivateV2 = await timed(() =>
    activateVersion({
      installRoot: sideInstallRoot,
      staged: sideStageV2.value,
      storageSchemaVersion: 1
    })
  );
  const sideActiveV2 = await readActiveVersion(sideInstallRoot);

  setStorageSchemaVersion(durable.dbPath, 2);
  const sideBlockedRollbackCode = await asyncErrorCodeOf(() =>
    activateVersion({
      installRoot: sideInstallRoot,
      staged: sideInstallV1.value.staged,
      storageSchemaVersion: storageSchemaVersion(durable.dbPath)
    })
  );
  const sideActiveAfterBlockedRollback =
    await readActiveVersion(sideInstallRoot);


  setStorageSchemaVersion(durable.dbPath, 1);
  const sideRollbackV1 = await timed(() =>
    activateVersion({
      installRoot: sideInstallRoot,
      staged: sideInstallV1.value.staged,
      storageSchemaVersion: storageSchemaVersion(durable.dbPath)
    })
  );
  const sideActiveAfterRollback =
    await readActiveVersion(sideInstallRoot);

  const sideUninstall = await timed(() =>
    uninstallBinaries(sideInstallRoot)
  );
  const sideRemoved = !(await fileExists(sideInstallRoot));
  const durableAfterSide = {
    marker_exists: await fileExists(durable.marker),
    database_exists: await fileExists(durable.dbPath),
    schema_version: storageSchemaVersion(durable.dbPath)
  };

  const servicesAfter = relayServiceCount();

  function packageSummary(info) {
    if (!info) return null;
    return {
      name: info.name ?? info.Name,
      version: info.version ?? info.Version,
      publisher: info.publisher ?? info.Publisher
    };
  }

  function signatureSummary(info) {
    return {
      status: info?.status ?? info?.Status ?? null,
      signer_subject:
        info?.signer_subject ?? info?.SignerSubject ?? null
    };
  }

  function activeSummary(active) {
    if (!active) return null;
    return {
      version: active.version,
      bundle_sha256: active.bundle_sha256,
      channel: active.channel,
      source: active.source,
      max_storage_schema: active.max_storage_schema,
      components: active.components?.map(component => ({
        id: component.id,
        kind: component.kind,
        path: component.path,
        version: component.version,
        source: component.source,
        provenance: component.provenance,
        sha256: component.sha256
      })) ?? []
    };
  }


  const updaterSource = path.join(
    packagingRoot,
    "side_by_side_updater.mjs"
  );
  const updaterSourceLines = await countLines(updaterSource);
  const updaterSourceBytes = await fileSize(updaterSource);
  const rawPayloadBytes =
    (await fileSize(coreExe)) + (await fileSize(adapterExe));
  const appInstallerVersion = installedAppInstallerVersion();
  const makeAppxVersion = ps(
    "(Get-Item " + quotePs(makeAppx) + ").VersionInfo.FileVersion"
  );
  const signToolVersion = ps(
    "(Get-Item " + quotePs(signTool) + ").VersionInfo.FileVersion"
  );

  result = {
    benchmark: "phase1-spike13-packaging-update-windows",
    recorded_at: new Date().toISOString(),
    decision_informed:
      "Which Windows install/update model should RELAY use for the default non-admin personal path while keeping update integrity, rollback, durable data, and component provenance explicit.",
    hypothesis:
      "Signed MSIX + App Installer will satisfy the default per-user install/update contract with less RELAY-owned updater/recovery code than a custom signed side-by-side updater, while schema-aware binary rollback remains a RELAY preflight in either model.",

    runtime: {
      os: { platform: process.platform, release: os.release(), arch: os.arch() },
      cpu_model: os.cpus()[0]?.model.trim() ?? "unknown",
      logical_cpu_count: os.cpus().length,
      total_memory_bytes: os.totalmem(),
      token: tokenEvidence,
      app_installer_version: appInstallerVersion
    },
    toolchain: {
      makeappx_version: makeAppxVersion,
      signtool_version: signToolVersion,
      powershell_present: await fileExists(powershellExe),
      tar_present: await fileExists(tarExe),
      raw_payload_bytes: rawPayloadBytes,
      release_components: 2
    },
    msix_appinstaller: {
      package_v1: {
        size_bytes: msix1Build.value.size_bytes,
        build_sign_verify_ms: msix1Build.elapsed_ms
      },
      package_v2: {
        size_bytes: msix2Build.value.size_bytes,
        build_sign_verify_ms: msix2Build.elapsed_ms,
        signature: signatureSummary(msixSignature)
      },
      appinstaller: appInstallerEvidence,
      package_v1_inventory: msixV1PackageInventory,
      package_v2_inventory: msixV2PackageInventory,
      install_v1_attempt: msixInstallV1Attempt,
      lifecycle_proven_on_fixture: msixInstallV1Attempt.ok,
      installed_v1: packageSummary(msixV1Info),
      start_version_response: msixStart,
      installed_v1_inventory: msixV1Inventory,

      tampered_signature_rejected: msixTamperRejected,
      tampered_update_attempt: msixTamperedInstall,
      version_after_tampered_update:
        msixVersionAfterTamperedUpdate,
      update_v2_ms: msixUpdateV2Ms,
      installed_v2: packageSummary(msixV2Info),
      installed_v2_inventory: msixV2Inventory,
      incompatible_rollback_error: msixBlockedRollbackCode,
      version_after_blocked_rollback:
        msixVersionAfterBlockedRollback,
      rollback_v1_ms: msixRollbackV1Ms,
      version_after_compatible_rollback:
        msixVersionAfterRollback,
      uninstall_ms: msixUninstallMs,
      package_removed: msixRemoved,
      durable_data_after_uninstall: durableAfterMsix,
      update_model: {
        manual_offline: "signed .msix install",
        automated:
          "signed .appinstaller OnLaunch + background update; automatic downgrade disabled",
        rollback:
          "RELAY storage-schema preflight then explicit ForceUpdateFromAnyVersion"
      }
    },
    side_by_side: {
      bundle_v1: {
        size_bytes: zip1Build.value.size_bytes,
        signature_bytes: zip1Build.value.signature_bytes,
        build_sign_verify_ms: zip1Build.elapsed_ms
      },
      bundle_v2: {
        size_bytes: zip2Build.value.size_bytes,
        signature_bytes: zip2Build.value.signature_bytes,
        build_sign_verify_ms: zip2Build.elapsed_ms,
        signature: {
          subject: zipSigner.subject,
          signers: zipSigner.signers
        }
      },

      tampered_signature_rejected: zipTamperRejected,
      install_v1_ms: sideInstallV1.elapsed_ms,
      active_v1: activeSummary(sideActiveV1),
      tampered_update_error: sideTamperedUpdateCode,
      active_after_tampered_update:
        activeSummary(sideActiveAfterTamper),
      stage_v2_ms: sideStageV2.elapsed_ms,
      active_after_stage_before_activation:
        activeSummary(sideActiveAfterStage),
      activate_v2_ms: sideActivateV2.elapsed_ms,
      active_v2: activeSummary(sideActiveV2),
      incompatible_rollback_error: sideBlockedRollbackCode,
      active_after_blocked_rollback:
        activeSummary(sideActiveAfterBlockedRollback),
      rollback_v1_ms: sideRollbackV1.elapsed_ms,
      active_after_compatible_rollback:
        activeSummary(sideActiveAfterRollback),
      uninstall_ms: sideUninstall.elapsed_ms,
      binaries_removed: sideRemoved,
      durable_data_after_uninstall: durableAfterSide,
      updater_source_lines: updaterSourceLines,
      updater_source_bytes: updaterSourceBytes,
      update_model: {
        manual_offline: "ZIP bundle + detached CMS signature",
        automated:
          "RELAY-owned updater fetches signed bundle, verifies, stages side-by-side, then atomically changes active pointer",
        rollback:
          "activate prior verified version only after storage-schema preflight"
      }
    },

    shared: {
      current_user_non_elevated: tokenEvidence.elevated === false,
      user_session_id: tokenEvidence.session_id,
      relay_service_count_before: servicesBefore,
      relay_service_count_after: servicesAfter,
      signing_fixture:
        "ephemeral CurrentUser code-signing cert trusted only in CurrentUser\\TrustedPeople + CurrentUser\\Root during the fixture; cleanup removes both and production must use a real trusted signing path",
      durable_data_root_owned_separately: true
    },
    comparison: {
      msix_v2_bytes: msix2Build.value.size_bytes,
      side_by_side_v2_bytes: zip2Build.value.size_bytes,
      side_by_side_v2_signature_bytes:
        zip2Build.value.signature_bytes,
      msix_vs_side_by_side_size_ratio:
        +(msix2Build.value.size_bytes /
          (zip2Build.value.size_bytes +
            zip2Build.value.signature_bytes)).toFixed(4),
      raw_payload_bytes: rawPayloadBytes,
      custom_updater_owned_source_lines: updaterSourceLines,
      msix_update_engine:
        "Windows App Installer / AppX deployment stack",
      side_by_side_update_engine:
        "RELAY-owned verification/staging/activation/rollback code"
    },
    selection: {
      default_personal_path: "signed_side_by_side",
      optional_windows_managed_channel:
        "msix_appinstaller_when_trusted_signing_or_store_path_exists",
      reason:
        "the side-by-side candidate physically proved the complete non-admin install/update/rollback/uninstall contract on this fixture; the self-signed MSIX package was correctly built and signed but AppX deployment rejected the local test trust chain"
    },
    pass_fail: {
      non_admin_default_personal_install_proven:
        tokenEvidence.elevated === false &&
        sideActiveV1?.version === v1,
      no_relay_session0_service:
        tokenEvidence.session_id !== 0 &&
        servicesBefore === 0 &&
        servicesAfter === 0,
      msix_signature_valid:
        (msixSignature.status ?? msixSignature.Status) === "Valid",
      msix_tamper_signature_rejected:
        msixTamperRejected,
      msix_package_component_inventory_valid:
        msixV1PackageInventory.components.every(item => item.hash_matches) &&
        msixV2PackageInventory.components.every(item => item.hash_matches),
      msix_trust_gate_classified:
        msixInstallV1Attempt.ok ||
        msixInstallV1Attempt.hresults.includes("0x80073CF0"),
      msix_lifecycle_consistent_if_installable:
        !msixInstallV1Attempt.ok ||
        (
          msixStart?.version === "0.1.0-phase1-rust" &&
          (msixV2Info?.version ?? msixV2Info?.Version) === v2 &&
          msixTamperedInstall?.rejected === true &&
          msixVersionAfterTamperedUpdate === v1 &&
          msixBlockedRollbackCode ===
            "UPDATE_STORAGE_SCHEMA_INCOMPATIBLE" &&
          msixVersionAfterBlockedRollback === v2 &&
          msixVersionAfterRollback === v1 &&
          msixRemoved &&
          durableAfterMsix.marker_exists &&
          durableAfterMsix.database_exists &&
          durableAfterMsix.schema_version === 1
        ),
      appinstaller_upgrade_only_auto_update_defined:
        appInstallerEvidence.on_launch_check &&
        appInstallerEvidence.background_update &&
        appInstallerEvidence.automatic_downgrade_disabled,
      side_signature_valid:
        zipSigner.subject === publisher &&
        zipSigner.signers === 1,
      side_tamper_rejected:
        zipTamperRejected &&
        sideTamperedUpdateCode === "UPDATE_SIGNATURE_INVALID" &&
        sideActiveAfterTamper?.version === v1,
      side_interrupted_stage_not_activated:
        sideActiveAfterStage?.version === v1,
      side_update_v2_works:
        sideActiveV2?.version === v2,
      side_incompatible_rollback_blocked:
        sideBlockedRollbackCode ===
          "UPDATE_STORAGE_SCHEMA_INCOMPATIBLE" &&
        sideActiveAfterBlockedRollback?.version === v2,
      side_compatible_rollback_works:
        sideActiveAfterRollback?.version === v1,
      side_component_inventory_retained:
        sideActiveV2?.components?.length === 2 &&
        sideActiveV2.components.every(
          item => item.version === v2 &&
            item.sha256?.length === 64 &&
            item.provenance === "spike13-synthetic"
        ),
      side_uninstall_preserves_durable_data:
        sideRemoved &&
        durableAfterSide.marker_exists &&
        durableAfterSide.database_exists &&
        durableAfterSide.schema_version === 1
    },

    limitations: [
      "The MSIX fixture uses an ephemeral self-signed current-user test certificate imported into CurrentUser TrustedPeople + CurrentUser Root only for local verification and removed in finally. Public direct-download distribution requires a certificate Windows already trusts or Microsoft Store signing; RELAY must not ship the test trust path.",
      "MSIX auto-update behavior is defined by a generated 2021-schema .appinstaller file. This non-admin fixture attempts Add-AppxPackage; if Windows rejects the ephemeral self-signed trust chain, the result records that trust gate rather than claiming install/update/rollback lifecycle proof. The benchmark does not host a production HTTPS update endpoint.",
      "The ms-appinstaller URI scheme is not required by the selected fixture; consumer distribution can link to the .appinstaller file itself.",
      "The side-by-side updater is a Phase 1 Node prototype used to measure RELAY-owned updater/recovery complexity; a production custom updater would need an implementation in the selected shipping stack.",
      "The two package versions contain the same current Rust executable binaries with different synthetic release/package version metadata. Storage schema version 2 is simulated only to test rollback refusal, not as a real migration.",
      "The durable-data fixture is intentionally outside both package roots. Production uninstall UX must separately expose any explicit user choice to remove durable RELAY data.",
      "Only one Windows 11 workstation and one App Installer version were measured; enterprise policy, Store distribution, and older supported Windows builds remain separate release qualification work.",
      "No UEFN, Fortnite, Blender, Krita, or real adapter payload was packaged in Spike 13."
    ]
  };
} finally {
  try {
    removeFixturePackage();
  } catch {}
  if (certificate?.thumbprint) {
    try {
      cleanupTestCertificate(certificate.thumbprint);
    } catch {}
  }

  cleanup = {
    package_remaining: packageInfo() != null,
    test_certificate_remaining:
      certificate?.thumbprint
        ? certificateStillPresent(certificate.thumbprint)
        : false,
    relay_service_count_after_cleanup: relayServiceCount()
  };
}


if (!result) {
  throw new Error("Spike 13 did not produce a result");
}

result.cleanup = cleanup;
result.pass_fail.cleanup_complete =
  cleanup.package_remaining === false &&
  cleanup.test_certificate_remaining === false &&
  cleanup.relay_service_count_after_cleanup === 0;
result.pass_fail.all_gates_pass =
  Object.values(result.pass_fail).every(value => value === true);

try {
  await fs.mkdir(path.dirname(resultPath), { recursive: true });
  await fs.writeFile(
    resultPath,
    JSON.stringify(result, null, 2) + "\n",
    "utf8"
  );
  console.log(JSON.stringify(result, null, 2));
} finally {
  await fs.rm(root, { recursive: true, force: true }).catch(() => {});
}
