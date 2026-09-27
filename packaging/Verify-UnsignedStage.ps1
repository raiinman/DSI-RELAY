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
$expected = @{}
foreach ($file in $manifest.files) {
    $relative = [string]$file.path
    Assert-Relative $relative
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
foreach ($required in @('relay.exe', 'relayd.exe', 'LICENSE', 'NOTICE-RELAY.txt',
    'Cargo.lock', 'THIRD-PARTY-INVENTORY.json', 'README.txt', 'verify-package.ps1')) {
    if (-not $expected.ContainsKey($required)) { throw "Required package file missing: $required" }
}
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
    if ($line -notmatch '^([0-9a-f]{64})  ([A-Za-z0-9._/-]+)$') { throw 'Malformed checksum row' }
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

$actual = @(Get-ChildItem -LiteralPath $root -File -Recurse | ForEach-Object {
    $_.FullName.Substring($root.Length + 1).Replace('\', '/')
})
if ($actual.Count -ne $expected.Count) { throw 'Unexpected or missing package file' }
foreach ($relative in $actual) {
    if (-not $expected.ContainsKey($relative)) { throw 'Unexpected package file' }
}
Write-Output "Verified unsigned folder: $($manifest.package_name) ($($expected.Count) files)"
