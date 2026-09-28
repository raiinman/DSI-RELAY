param([string]$PackageDirectory)

$ErrorActionPreference = 'Stop'
if (-not $PackageDirectory) { $PackageDirectory = $PSScriptRoot }
$root = [IO.Path]::GetFullPath($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($PackageDirectory))
$rootItem = Get-Item -LiteralPath $root -Force -ErrorAction Stop
if (-not $rootItem.PSIsContainer -or ($rootItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'Package directory must be a regular directory'
}

function Get-Hash([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
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
            throw 'Package file contains a personal profile path or account identifier'
        }
    }
}

function Assert-Relative([string]$Relative) {
    if (-not $Relative -or $Relative -notmatch '^[A-Za-z0-9._/-]+$' -or
        $Relative.StartsWith('/') -or
        @($Relative.Split('/') | Where-Object { $_ -eq '' -or $_ -eq '.' -or $_ -eq '..' }).Count -gt 0) {
        throw 'Unsafe package path'
    }
}

$manifestPath = Join-Path $root 'bundle-manifest.json'
$sumsPath = Join-Path $root 'SHA256SUMS.txt'
foreach ($path in @($manifestPath, $sumsPath)) {
    $item = Get-Item -LiteralPath $path -Force -ErrorAction Stop
    if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Package metadata is not a regular file'
    }
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ($manifest.schema_version -ne 1 -or $manifest.status -ne 'unsigned_unpublished_stage' -or
    $manifest.platform -ne 'windows-x64' -or
    [int]$manifest.storage_schema_min -lt 1 -or
    [int]$manifest.storage_schema_max -lt [int]$manifest.storage_schema_min) {
    throw 'Unsupported package manifest'
}
$runtimeBinaries = @('relay.exe', 'relayd.exe', 'relay-gateway.exe')
$runtimeSkillFiles = @(
    'skills/relay-core/SKILL.md',
    'skills/relay-core/scripts/relay-core.ps1',
    'skills/relay-core/references/commands.generated.json'
)
$runtimePluginFiles = @(
    'plugins/dsi-relay-chat/plugin.json',
    'plugins/dsi-relay-chat/mcp.json',
    'plugins/dsi-relay-chat/.codex-plugin/plugin.json',
    'plugins/dsi-relay-chat/scripts/start-relay-chat.ps1',
    'plugins/dsi-relay-chat/README.md'
)
$runtimeComponents = @($runtimeBinaries + $runtimeSkillFiles + $runtimePluginFiles)
if (@($manifest.runtime_components).Count -ne $runtimeComponents.Count -or
    (@(Compare-Object -ReferenceObject $runtimeComponents -DifferenceObject @($manifest.runtime_components) -CaseSensitive).Count -ne 0)) {
    throw 'Runtime component inventory is incomplete or unexpected'
}
$archiveName = "relay-$($manifest.version)-windows-x64-unsigned-stage"
if ([string]$manifest.version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$' -or
    $archiveName.Length -gt 96) { throw 'Package version makes archive paths invalid' }
if (@($manifest.files).Count -gt 510) { throw 'Package exceeds archive entry count limit' }
$expected = @{}
foreach ($file in $manifest.files) {
    $relative = [string]$file.path
    Assert-Relative $relative
    if (("$archiveName/$relative").Length -gt 256) { throw 'Package archive entry path is too long' }
    if ($expected.ContainsKey($relative)) { throw 'Duplicate package file in manifest' }
    $segments = $relative.Split('/')
    $current = $root
    for ($index = 0; $index -lt ($segments.Length - 1); $index++) {
        $current = Join-Path $current $segments[$index]
        $directory = Get-Item -LiteralPath $current -Force -ErrorAction Stop
        if (-not $directory.PSIsContainer -or ($directory.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw 'Package contains a redirected directory'
        }
    }
    $path = Join-Path $root ($relative.Replace('/', '\'))
    $item = Get-Item -LiteralPath $path -Force -ErrorAction Stop
    if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Package payload is not a regular file'
    }
    if ([IO.Path]::GetFullPath($path).StartsWith($root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -eq $false) {
        throw 'Package path escapes root'
    }
    if ($item.Length -ne [int64]$file.bytes -or (Get-Hash $path) -ne [string]$file.sha256) {
        throw "Package file hash or size mismatch: $relative"
    }
    $expected[$relative] = (Get-Hash $path)
}
foreach ($required in @($runtimeComponents + @('LICENSE', 'NOTICE-RELAY.txt',
    'Cargo.lock', 'THIRD-PARTY-INVENTORY.json', 'README.txt', 'verify-package.ps1'))) {
    if (-not $expected.ContainsKey($required)) { throw "Required package file missing: $required" }
}
foreach ($binaryName in $runtimeBinaries) {
    $binaryPath = Join-Path $root $binaryName
    Assert-WindowsX64 $binaryPath
    Assert-NoPersonalPath $binaryPath
}
foreach ($relative in $runtimeSkillFiles) {
    Assert-NoPersonalPath (Join-Path $root ($relative.Replace('/', '\')))
}
foreach ($relative in $runtimePluginFiles) {
    Assert-NoPersonalPath (Join-Path $root ($relative.Replace('/', '\')))
}
$actualSkillFiles = @($manifest.files | ForEach-Object { [string]$_.path } | Where-Object { $_.StartsWith('skills/', [StringComparison]::Ordinal) })
if ($actualSkillFiles.Count -ne $runtimeSkillFiles.Count) { throw 'Unexpected runtime skill file' }
$actualPluginFiles = @($manifest.files | ForEach-Object { [string]$_.path } | Where-Object { $_.StartsWith('plugins/', [StringComparison]::Ordinal) })
if ($actualPluginFiles.Count -ne $runtimePluginFiles.Count) { throw 'Unexpected runtime plugin file' }
if ((Get-Hash (Join-Path $root 'Cargo.lock')) -ne [string]$manifest.cargo_lock_sha256) {
    throw 'Cargo.lock digest mismatch'
}
$licenseText = Get-Content -LiteralPath (Join-Path $root 'LICENSE') -Raw
$noticeText = Get-Content -LiteralPath (Join-Path $root 'NOTICE-RELAY.txt') -Raw
if (-not $licenseText.Contains('MIT License') -or -not $licenseText.Contains('RAiiNMAN') -or
    -not $noticeText.Contains('Created by RAiiNMAN')) {
    throw 'MIT license or RAiiNMAN credit missing'
}

$expected['bundle-manifest.json'] = Get-Hash $manifestPath
$seenSums = @{}
foreach ($line in Get-Content -LiteralPath $sumsPath) {
    if (-not ($line -match '^([0-9a-f]{64})  ([A-Za-z0-9._/-]+)$')) { throw 'Malformed checksum row' }
    $relative = $Matches[2]
    Assert-Relative $relative
    if ($seenSums.ContainsKey($relative) -or -not $expected.ContainsKey($relative) -or
        $expected[$relative] -ne $Matches[1]) {
        throw 'Checksum list mismatch'
    }
    $seenSums[$relative] = $true
}
if ($seenSums.Count -ne $expected.Count) { throw 'Checksum list is incomplete' }
$expected['SHA256SUMS.txt'] = Get-Hash $sumsPath

$directories = @(Get-ChildItem -LiteralPath $root -Directory -Recurse -Force)
if (@($directories | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }).Count -gt 0) {
    throw 'Package contains a redirected directory'
}
$actualFiles = @(Get-ChildItem -LiteralPath $root -File -Recurse -Force)
$actual = @($actualFiles | ForEach-Object {
    $_.FullName.Substring($root.Length + 1).Replace('\', '/')
})
if ($actual.Count -gt 512 -or (($actualFiles | Measure-Object -Property Length -Sum).Sum -gt 256MB)) {
    throw 'Package exceeds archive entry count or expansion limit'
}
if ($actual.Count -ne $expected.Count) { throw 'Unexpected or missing package file' }
foreach ($relative in $actual) {
    if (-not $expected.ContainsKey($relative)) { throw 'Unexpected package file' }
}
Write-Output "Verified unsigned folder: $($manifest.package_name) ($($expected.Count) files)"
