param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$')]
    [string]$Version,
    [Parameter(Mandatory = $true)]
    [string]$BinaryDirectory,
    [Parameter(Mandatory = $true)]
    [ValidateRange(1, 10000)]
    [int]$MinimumStorageSchema,
    [Parameter(Mandatory = $true)]
    [ValidateRange(1, 10000)]
    [int]$MaximumStorageSchema,
    [string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
if ($MaximumStorageSchema -lt $MinimumStorageSchema) { throw 'Storage schema range is invalid' }
$repoRoot = [IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $PSScriptRoot 'out' }
$binaryRoot = [IO.Path]::GetFullPath($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($BinaryDirectory))
$outputRoot = [IO.Path]::GetFullPath($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory))
$name = "relay-$Version-windows-x64-unsigned-stage"
if ($name.Length -gt 96) { throw 'Version makes archive entry paths too long' }
$finalFolder = Join-Path $outputRoot $name
$finalArchive = Join-Path $outputRoot "$name.zip"
$archiveHashFile = "$finalArchive.sha256"
$stageName = '.relay-stage-' + [Guid]::NewGuid().ToString('N')
$stageRoot = Join-Path $outputRoot $stageName
$payloadRoot = Join-Path $stageRoot $name
$runtimeBinaries = @('relay.exe', 'relayd.exe', 'relay-gateway.exe')
$runtimeSkillFiles = @(
    'skills/relay-core/SKILL.md',
    'skills/relay-core/scripts/relay-core.ps1',
    'skills/relay-core/references/commands.generated.json'
)

function Get-Hash([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Write-Utf8Lf([string]$Path, [string]$Value) {
    $encoding = New-Object Text.UTF8Encoding($false)
    [IO.File]::WriteAllText($Path, $Value.Replace("`r`n", "`n"), $encoding)
}

function Assert-RegularFile([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Package input must be a regular file'
    }
}

function Assert-WindowsX64([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    try {
        $reader = New-Object IO.BinaryReader($stream)
        if ($reader.ReadUInt16() -ne 0x5a4d) { throw 'Package binary is not a Windows executable' }
        $stream.Position = 0x3c
        $offset = $reader.ReadInt32()
        if ($offset -lt 0x40 -or $offset -gt ($stream.Length - 6)) { throw 'Invalid PE header' }
        $stream.Position = $offset
        if ($reader.ReadUInt32() -ne 0x00004550 -or $reader.ReadUInt16() -ne 0x8664) {
            throw 'Package binary is not Windows x64'
        }
    }
    finally { $stream.Dispose() }
}

function Assert-NoPersonalPath([string]$Path) {
    $bytes = [IO.File]::ReadAllBytes($Path)
    $single = [Text.Encoding]::GetEncoding(28591).GetString($bytes)
    $wide = [Text.Encoding]::Unicode.GetString($bytes)
    $needles = @($env:USERPROFILE, $env:USERNAME, 'C:\Users\', 'C:/Users/') |
        Where-Object { $_ -and $_.Length -ge 3 } | Select-Object -Unique
    foreach ($needle in $needles) {
        if ($single.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -ge 0 -or
            $wide.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw 'Package binary contains a personal profile path or account identifier'
        }
    }
}

function Get-RuntimePackages($Metadata) {
    $roots = @($Metadata.packages | Where-Object { $_.name -in @('relay', 'relayd', 'relay-gateway') } | ForEach-Object { $_.id })
    if ($roots.Count -ne 3) { throw 'Could not identify CLI, daemon, and gateway dependency roots' }
    $nodes = @{}
    foreach ($node in $Metadata.resolve.nodes) { $nodes[[string]$node.id] = $node }
    $seen = @{}
    $queue = New-Object 'Collections.Generic.Queue[string]'
    foreach ($id in $roots) { $queue.Enqueue([string]$id) }
    while ($queue.Count -gt 0) {
        $id = $queue.Dequeue()
        if ($seen.ContainsKey($id)) { continue }
        $seen[$id] = $true
        $node = $nodes[$id]
        if (-not $node) { throw 'Dependency graph is incomplete' }
        foreach ($dep in $node.deps) {
            if (@($dep.dep_kinds | Where-Object { $_.kind -ne 'dev' }).Count -gt 0) {
                $queue.Enqueue([string]$dep.pkg)
            }
        }
    }
    @($Metadata.packages | Where-Object {
        $seen.ContainsKey([string]$_.id) -and $_.source -like 'registry+*'
    } | Sort-Object name, version)
}

function Get-LicenseSources($Package) {
    $crateRoot = Split-Path ([string]$Package.manifest_path) -Parent
    $found = @(Get-ChildItem -LiteralPath $crateRoot -File | Where-Object {
        $_.Name -match '^(?:LICENSE|LICENCE|COPYING|NOTICE|UNLICENSE|AUTHORS|COPYRIGHT)(?:[._-].*)?$'
    })
    if ($Package.license_file) {
        $declared = [IO.Path]::GetFullPath((Join-Path $crateRoot ([string]$Package.license_file)))
        if (-not $declared.StartsWith($crateRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Dependency license file escapes its crate'
        }
        Assert-RegularFile $declared
        $found += Get-Item -LiteralPath $declared
    }
    @($found | Sort-Object FullName -Unique)
}

if (-not (Test-Path -LiteralPath $binaryRoot -PathType Container)) { throw 'Binary directory does not exist' }
$binaryItem = Get-Item -LiteralPath $binaryRoot -Force
if ($binaryItem.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Binary directory is a reparse point' }
foreach ($binaryName in $runtimeBinaries) {
    $path = Join-Path $binaryRoot $binaryName
    Assert-RegularFile $path
    Assert-WindowsX64 $path
    Assert-NoPersonalPath $path
}
foreach ($relativeDirectory in @('skills', 'skills\relay-core', 'skills\relay-core\scripts', 'skills\relay-core\references')) {
    $directory = Get-Item -LiteralPath (Join-Path $repoRoot $relativeDirectory) -Force -ErrorAction Stop
    if (-not $directory.PSIsContainer -or ($directory.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Runtime skill source directory is redirected'
    }
}
foreach ($relative in $runtimeSkillFiles) {
    $path = Join-Path $repoRoot ($relative.Replace('/', '\'))
    Assert-RegularFile $path
    Assert-NoPersonalPath $path
}
foreach ($source in @('LICENSE', 'NOTICE-RELAY.txt', 'Cargo.lock')) {
    Assert-RegularFile (Join-Path $repoRoot $source)
}
$licenseText = Get-Content -LiteralPath (Join-Path $repoRoot 'LICENSE') -Raw
$noticeText = Get-Content -LiteralPath (Join-Path $repoRoot 'NOTICE-RELAY.txt') -Raw
if (-not $licenseText.Contains('MIT License') -or -not $licenseText.Contains('RAiiNMAN') -or
    -not $noticeText.Contains('Created by RAiiNMAN')) {
    throw 'MIT license or RAiiNMAN credit is missing'
}

Push-Location $repoRoot
try {
    $metadataText = & cargo metadata --locked --offline --format-version 1 --filter-platform x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Offline dependency inventory failed' }
    $metadata = $metadataText | ConvertFrom-Json
}
finally { Pop-Location }
$packages = @(Get-RuntimePackages $metadata)

if (-not (Test-Path -LiteralPath $outputRoot)) {
    $null = New-Item -ItemType Directory -Path $outputRoot
}
$outputItem = Get-Item -LiteralPath $outputRoot -Force
if (-not $outputItem.PSIsContainer -or ($outputItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'Output must be a regular directory'
}
if ((Test-Path -LiteralPath $finalFolder) -or (Test-Path -LiteralPath $finalArchive) -or
    (Test-Path -LiteralPath $archiveHashFile) -or (Test-Path -LiteralPath $stageRoot)) {
    throw 'Versioned artifact or staging path already exists'
}
$stageFull = [IO.Path]::GetFullPath($stageRoot)
if ([IO.Path]::GetDirectoryName($stageFull) -ne $outputRoot -or
    [IO.Path]::GetFileName($stageFull) -ne $stageName -or
    $stageName -notmatch '^\.relay-stage-[0-9a-f]{32}$') {
    throw 'Staging path failed validation'
}

$null = New-Item -ItemType Directory -Path $payloadRoot
foreach ($nameToCopy in @('LICENSE', 'NOTICE-RELAY.txt', 'Cargo.lock')) {
    Copy-Item -LiteralPath (Join-Path $repoRoot $nameToCopy) -Destination (Join-Path $payloadRoot $nameToCopy)
}
foreach ($binaryName in $runtimeBinaries) {
    $source = Join-Path $binaryRoot $binaryName
    $before = Get-Hash $source
    Copy-Item -LiteralPath $source -Destination (Join-Path $payloadRoot $binaryName)
    if ((Get-Hash $source) -ne $before -or (Get-Hash (Join-Path $payloadRoot $binaryName)) -ne $before) {
        throw 'Release binary changed during assembly'
    }
}
foreach ($relative in $runtimeSkillFiles) {
    $source = Join-Path $repoRoot ($relative.Replace('/', '\'))
    $destination = Join-Path $payloadRoot ($relative.Replace('/', '\'))
    $before = Get-Hash $source
    $null = New-Item -ItemType Directory -Path (Split-Path $destination -Parent) -Force
    Copy-Item -LiteralPath $source -Destination $destination
    if ((Get-Hash $source) -ne $before -or (Get-Hash $destination) -ne $before) {
        throw 'Runtime skill file changed during assembly'
    }
}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'Verify-UnsignedStage.ps1') -Destination (Join-Path $payloadRoot 'verify-package.ps1')

$inventory = @()
$unresolved = New-Object 'Collections.Generic.List[string]'
foreach ($package in $packages) {
    $label = '{0}-{1}' -f $package.name, $package.version
    if ($label -notmatch '^[A-Za-z0-9._-]+$') { throw 'Unsafe dependency label' }
    $sources = @(Get-LicenseSources $package)
    if ($sources.Count -eq 0) { $unresolved.Add("NO_LICENSE_TEXT:$label") }
    $files = @()
    foreach ($source in $sources) {
        $relative = "third-party/$label/$($source.Name)"
        if ($source.Name -notmatch '^[A-Za-z0-9._-]+$' -or $files -contains $relative) {
            throw 'Duplicate or unsafe dependency notice filename'
        }
        $destination = Join-Path $payloadRoot ($relative.Replace('/', '\'))
        $null = New-Item -ItemType Directory -Path (Split-Path $destination -Parent) -Force
        Copy-Item -LiteralPath $source.FullName -Destination $destination
        $files += [ordered]@{ path = $relative; sha256 = Get-Hash $destination }
    }
    $inventory += [ordered]@{
        crate = [string]$package.name
        version = [string]$package.version
        license_expression = [string]$package.license
        copied_license_texts = @($files)
    }
}
Write-Utf8Lf (Join-Path $payloadRoot 'THIRD-PARTY-INVENTORY.json') (($inventory | ConvertTo-Json -Depth 6) + "`n")

$readme = @"
DSI RELAY $Version - UNSIGNED LOCAL STAGE

Created by RAiiNMAN. RELAY first-party source uses the MIT license in LICENSE.
Third-party license texts and expressions are in THIRD-PARTY-INVENTORY.json
and third-party/. Their legal completeness must be reviewed for this exact graph.

This package has not been signed, installed, live-tested, or approved for public
distribution. It does not contain an installer or updater. Do not run it as a
published product. relay.exe, relayd.exe, and relay-gateway.exe are staged for
local integration work. The compact local coding-agent skill is in
skills/relay-core/; its script calls the staged relay CLI through the shared
command system. The gateway is an optional local MCP adapter; it is not a
public or remote endpoint.

When using the staged skill from this folder, pass -RelayPath .\relay.exe to
its scripts/relay-core.ps1 wrapper (or point it to an installed RELAY CLI).

Verify the folder before use:
  powershell -NoProfile -ExecutionPolicy Bypass -File .\verify-package.ps1

No telemetry or upload runs during package assembly or verification.
The per-user install/update/uninstall design is maintained in the source
repository's packaging/INSTALL-UPDATE-PLAN.md.
"@
Write-Utf8Lf (Join-Path $payloadRoot 'README.txt') ($readme + "`n")
$unresolved.Add('THIRD_PARTY_NOTICE_LEGAL_REVIEW')
$unresolved.Add('PUBLISHER_SIGNATURE_AND_TRUST_PENDING')
$unresolved.Add('CLEAN_ACCOUNT_INSTALL_TEST_PENDING')
$unresolved.Add('INTEGRATED_WORKFLOW_TEST_PENDING')
$unresolved.Add('PUBLIC_RELEASE_APPROVAL_PENDING')

$files = @(Get-ChildItem -LiteralPath $payloadRoot -File -Recurse | ForEach-Object {
    $relative = $_.FullName.Substring($payloadRoot.Length + 1).Replace('\', '/')
    [ordered]@{ path = $relative; bytes = [int64]$_.Length; sha256 = Get-Hash $_.FullName }
} | Sort-Object path)
$manifest = [ordered]@{
    schema_version = 1
    package_name = "relay-$Version-windows-x64"
    version = $Version
    platform = 'windows-x64'
    status = 'unsigned_unpublished_stage'
    storage_schema_min = $MinimumStorageSchema
    storage_schema_max = $MaximumStorageSchema
    source_binary_provenance = 'Supplied release binaries; binary-to-source correspondence not independently attested'
    cargo_lock_sha256 = Get-Hash (Join-Path $payloadRoot 'Cargo.lock')
    third_party_crate_count = $packages.Count
    runtime_components = @($runtimeBinaries + $runtimeSkillFiles)
    unresolved_checks = @($unresolved | Sort-Object -Unique)
    files = $files
}
Write-Utf8Lf (Join-Path $payloadRoot 'bundle-manifest.json') (($manifest | ConvertTo-Json -Depth 8) + "`n")
$sums = @(Get-ChildItem -LiteralPath $payloadRoot -File -Recurse | ForEach-Object {
    $relative = $_.FullName.Substring($payloadRoot.Length + 1).Replace('\', '/')
    '{0}  {1}' -f (Get-Hash $_.FullName), $relative
} | Sort-Object)
Write-Utf8Lf (Join-Path $payloadRoot 'SHA256SUMS.txt') (($sums -join "`n") + "`n")

$archiveFiles = @(Get-ChildItem -LiteralPath $payloadRoot -File -Recurse | Sort-Object FullName)
if ($archiveFiles.Count -gt 512 -or (($archiveFiles | Measure-Object -Property Length -Sum).Sum -gt 256MB)) {
    throw 'Staged archive exceeds entry count or expansion limit'
}
foreach ($file in $archiveFiles) {
    $relative = $file.FullName.Substring($payloadRoot.Length + 1).Replace('\', '/')
    if (("$name/$relative").Length -gt 256) { throw 'Staged archive entry path is too long' }
}

& (Join-Path $PSScriptRoot 'Verify-UnsignedStage.ps1') -PackageDirectory $payloadRoot
if ($LASTEXITCODE -ne 0) { throw 'Staged folder verification failed' }

Add-Type -AssemblyName System.IO.Compression
$zipPath = Join-Path $stageRoot "$name.zip"
$stream = [IO.File]::Open($zipPath, [IO.FileMode]::CreateNew)
try {
    $zip = New-Object IO.Compression.ZipArchive($stream, [IO.Compression.ZipArchiveMode]::Create, $false)
    try {
        foreach ($file in $archiveFiles) {
            $relative = $file.FullName.Substring($payloadRoot.Length + 1).Replace('\', '/')
            $entry = $zip.CreateEntry("$name/$relative", [IO.Compression.CompressionLevel]::Optimal)
            $entry.LastWriteTime = [DateTimeOffset]::Parse('2000-01-01T00:00:00+00:00')
            $entryStream = $entry.Open()
            try {
                $input = [IO.File]::OpenRead($file.FullName)
                try { $input.CopyTo($entryStream) }
                finally { $input.Dispose() }
            }
            finally { $entryStream.Dispose() }
        }
    }
    finally { $zip.Dispose() }
}
finally { $stream.Dispose() }
$zipHash = Get-Hash $zipPath
Move-Item -LiteralPath $payloadRoot -Destination $finalFolder
Move-Item -LiteralPath $zipPath -Destination $finalArchive
Write-Utf8Lf $archiveHashFile ("$zipHash  $name.zip`n")
$stageItem = Get-Item -LiteralPath $stageRoot -Force
if (-not $stageItem.PSIsContainer -or ($stageItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
    @(Get-ChildItem -LiteralPath $stageRoot -Force).Count -ne 0) {
    throw 'Staging directory was not empty after assembly'
}
Remove-Item -LiteralPath $stageRoot
Write-Output "Unsigned stage: $finalFolder"
Write-Output "Archive: $finalArchive"
Write-Output "Archive SHA-256: $zipHash"
Write-Output "Unresolved checks: $($manifest.unresolved_checks -join ', ')"
