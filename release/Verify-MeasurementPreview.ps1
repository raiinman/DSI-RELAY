param([string]$BundleDirectory)

$ErrorActionPreference = 'Stop'
if (-not $BundleDirectory) { $BundleDirectory = $PSScriptRoot }
$root = [System.IO.Path]::GetFullPath($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($BundleDirectory)).TrimEnd('\')
$manifestPath = Join-Path $root 'bundle-manifest.json'
$sumsPath = Join-Path $root 'SHA256SUMS.txt'
if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf) -or
    -not (Test-Path -LiteralPath $sumsPath -PathType Leaf)) {
    throw 'Bundle manifest or hash list is missing'
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ($manifest.schema_version -ne 1 -or $manifest.bundle_name -ne [System.IO.Path]::GetFileName($root)) {
    throw 'Bundle manifest schema or folder name does not match'
}

function Resolve-BundleFile([string]$Relative) {
    if (-not $Relative -or $Relative -match '(^/|^\\|:|\\|(^|/)\.\.?(/|$))') {
        throw "Invalid bundle-relative path: $Relative"
    }
    $full = [System.IO.Path]::GetFullPath((Join-Path $root $Relative))
    if (-not $full.StartsWith($root + [System.IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Bundle path escapes its root: $Relative"
    }
    if (-not (Test-Path -LiteralPath $full -PathType Leaf)) { throw "Missing bundle file: $Relative" }
    $item = Get-Item -LiteralPath $full -Force
    if ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) {
        throw "Bundle file is a reparse point: $Relative"
    }
    return $full
}

$listed = @{}
foreach ($entry in $manifest.files) {
    $path = [string]$entry.path
    if ($listed.ContainsKey($path)) { throw "Duplicate manifest path: $path" }
    $listed[$path] = $true
    $full = Resolve-BundleFile $path
    $item = Get-Item -LiteralPath $full
    if ($item.Length -ne [int64]$entry.bytes -or
        (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant() -ne [string]$entry.sha256) {
        throw "Manifest size or hash mismatch: $path"
    }
}
$sums = @{}
foreach ($line in Get-Content -LiteralPath $sumsPath) {
    if ($line -notmatch '^([0-9a-f]{64})  (.+)$') { throw 'Malformed SHA256SUMS line' }
    $digest = $Matches[1]
    $path = $Matches[2]
    if ($sums.ContainsKey($path)) { throw "Duplicate hash path: $path" }
    $sums[$path] = $true
    $full = Resolve-BundleFile $path
    if ((Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant() -ne $digest) {
        throw "SHA256SUMS mismatch: $path"
    }
}
if (-not $sums.ContainsKey('bundle-manifest.json') -or $sums.Count -ne $listed.Count + 1) {
    throw 'Hash list does not match manifest payload'
}
foreach ($path in $listed.Keys) {
    if (-not $sums.ContainsKey($path)) { throw "Payload missing from hash list: $path" }
}
$actualFiles = @(Get-ChildItem -LiteralPath $root -File -Recurse)
if ($actualFiles.Count -ne $sums.Count + 1) {
    throw 'Bundle has an unlisted or missing file'
}
Write-Output "Verified $($sums.Count) hashed bundle files; status: $($manifest.status)"
