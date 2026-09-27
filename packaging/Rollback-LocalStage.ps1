param(
    [string]$InstallRoot,
    [string]$DataRoot,
    [switch]$AllowUnsignedLocalDevelopment,
    [switch]$FixtureMode,
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$')]
    [string]$Version,
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-fA-F]{64}$')]
    [string]$ExpectedArchiveSha256,
    [ValidateRange(0, 10000)]
    [int]$ObservedStorageSchema
)

$ErrorActionPreference = 'Stop'
if (-not $AllowUnsignedLocalDevelopment) {
    throw 'Unsigned rollback requires -AllowUnsignedLocalDevelopment'
}
if ($DataRoot -and -not $FixtureMode) { throw 'Custom data roots require -FixtureMode' }
if ($FixtureMode -and -not $DataRoot) { throw 'Fixture rollback requires an explicit fixture data root' }
if (-not $FixtureMode -and $env:RELAY_STATE_DIR) {
    throw 'Local rollback cannot infer the data root while RELAY_STATE_DIR is set'
}
if (-not $DataRoot) {
    if (-not $env:LOCALAPPDATA) { throw 'LOCALAPPDATA is required for local rollback' }
    $DataRoot = Join-Path $env:LOCALAPPDATA 'DSI\RELAY'
}
if (-not $InstallRoot) { $InstallRoot = Join-Path $env:LOCALAPPDATA 'Programs\DSI-RELAY' }
$root = [IO.Path]::GetFullPath($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($InstallRoot))
$defaultRoot = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Programs\DSI-RELAY'))
if (-not $FixtureMode -and $root -ne $defaultRoot) { throw 'Custom install roots require -FixtureMode' }
if ($root -eq [IO.Path]::GetPathRoot($root) -or $root.Length -lt 12) { throw 'Install root is too broad' }

function Assert-NoReparseAncestors([string]$Path) {
    $cursor = [IO.Path]::GetFullPath($Path)
    while ($cursor) {
        if (Test-Path -LiteralPath $cursor) {
            if ((Get-Item -LiteralPath $cursor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw 'Install path contains a reparse point'
            }
        }
        $parent = [IO.Path]::GetDirectoryName($cursor)
        if (-not $parent -or $parent -eq $cursor) { break }
        $cursor = $parent
    }
}
function Assert-Directory([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Install path is not a regular directory'
    }
}
function Assert-RegularFile([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Install path is not a regular file'
    }
}
function Assert-DirectChild([string]$Child, [string]$Parent) {
    $full = [IO.Path]::GetFullPath($Child)
    $base = [IO.Path]::GetFullPath($Parent)
    if ([IO.Path]::GetDirectoryName($full) -ne $base) { throw 'Version path escapes install root' }
}
function Write-JsonAtomic([string]$Path, $Value) {
    Assert-RegularFile $Path
    $backup = "$Path.bak"
    if (Test-Path -LiteralPath $backup) { Assert-RegularFile $backup }
    $temporary = "$Path.new-$([Guid]::NewGuid().ToString('N'))"
    $bytes = (New-Object Text.UTF8Encoding($false)).GetBytes(($Value | ConvertTo-Json -Depth 6) + "`n")
    $stream = [IO.File]::Open($temporary, [IO.FileMode]::CreateNew)
    try {
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
    }
    finally { $stream.Dispose() }
    [IO.File]::Replace($temporary, $Path, $backup)
}

Assert-NoReparseAncestors $root
Assert-Directory $root
$markerPath = Join-Path $root 'install-root.json'
Assert-RegularFile $markerPath
$marker = Get-Content -LiteralPath $markerPath -Raw | ConvertFrom-Json
if ($marker.schema_version -ne 1 -or $marker.owner -ne 'relay_local_development') {
    throw 'Install root marker is invalid'
}
$versionsRoot = Join-Path $root 'versions'
$receiptsRoot = Join-Path $root 'receipts'
Assert-Directory $versionsRoot
Assert-Directory $receiptsRoot
$currentPath = Join-Path $root 'current.json'
Assert-RegularFile $currentPath
$current = Get-Content -LiteralPath $currentPath -Raw | ConvertFrom-Json
if ($current.schema_version -ne 1 -or $current.channel -ne 'unsigned_local_development' -or
    [string]$current.active_version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$' -or
    [string]$current.previous_version -ne $Version -or
    [string]$current.archive_sha256 -notmatch '^[0-9a-f]{64}$') {
    throw 'Current version pointer does not name the requested previous version'
}
if ([string]$current.active_version -eq $Version) {
    throw 'Requested rollback version is already active'
}
if (@(Get-Process -Name relayd -ErrorAction SilentlyContinue).Count -gt 0) {
    throw 'relayd.exe is running; rollback must wait until it stops'
}
$currentReceiptPath = Join-Path $receiptsRoot "$($current.active_version).json"
Assert-DirectChild $currentReceiptPath $receiptsRoot
Assert-RegularFile $currentReceiptPath
$currentReceipt = Get-Content -LiteralPath $currentReceiptPath -Raw | ConvertFrom-Json
if ($currentReceipt.schema_version -ne 1 -or
    $currentReceipt.version -ne $current.active_version -or
    $currentReceipt.archive_sha256 -ne $current.archive_sha256 -or
    $currentReceipt.channel -ne 'unsigned_local_development') {
    throw 'Current version pointer does not match its receipt'
}
$currentVersionPath = Join-Path $versionsRoot ([string]$current.active_version)
Assert-DirectChild $currentVersionPath $versionsRoot
Assert-Directory $currentVersionPath
& (Join-Path $PSScriptRoot 'Verify-UnsignedStage.ps1') -PackageDirectory $currentVersionPath
$currentManifestPath = Join-Path $currentVersionPath 'bundle-manifest.json'
Assert-RegularFile $currentManifestPath
$currentManifest = Get-Content -LiteralPath $currentManifestPath -Raw | ConvertFrom-Json
if ($currentManifest.version -ne $current.active_version -or
    [int]$currentManifest.storage_schema_min -ne [int]$currentReceipt.storage_schema_min -or
    [int]$currentManifest.storage_schema_max -ne [int]$currentReceipt.storage_schema_max -or
    ($currentReceipt.PSObject.Properties.Name -contains 'manifest_sha256' -and
        [string]$currentReceipt.manifest_sha256 -ne
            (Get-FileHash -LiteralPath $currentManifestPath -Algorithm SHA256).Hash.ToLowerInvariant())) {
    throw 'Current version manifest does not match its receipt'
}

$versionPath = Join-Path $versionsRoot $Version
$receiptPath = Join-Path $receiptsRoot "$Version.json"
Assert-DirectChild $versionPath $versionsRoot
Assert-DirectChild $receiptPath $receiptsRoot
Assert-Directory $versionPath
Assert-RegularFile $receiptPath
$receipt = Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json
if ($receipt.schema_version -ne 1 -or $receipt.version -ne $Version -or
    $receipt.channel -ne 'unsigned_local_development' -or
    [string]$receipt.archive_sha256 -ne $ExpectedArchiveSha256.ToLowerInvariant() -or
    [int]$receipt.storage_schema_min -lt 1 -or
    [int]$receipt.storage_schema_max -lt [int]$receipt.storage_schema_min) {
    throw 'Previous version receipt or expected archive digest is invalid'
}
& (Join-Path $PSScriptRoot 'Verify-UnsignedStage.ps1') -PackageDirectory $versionPath
$manifestPath = Join-Path $versionPath 'bundle-manifest.json'
Assert-RegularFile $manifestPath
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ($manifest.version -ne $Version -or
    [int]$manifest.storage_schema_min -ne [int]$receipt.storage_schema_min -or
    [int]$manifest.storage_schema_max -ne [int]$receipt.storage_schema_max) {
    throw 'Previous version manifest does not match its receipt'
}
if ($receipt.PSObject.Properties.Name -contains 'manifest_sha256' -and
    [string]$receipt.manifest_sha256 -ne (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash.ToLowerInvariant()) {
    throw 'Previous version manifest digest does not match its receipt'
}

$observedStorageSchema = & (Join-Path $PSScriptRoot 'Read-StorageSchema.ps1') `
    -RelaydPath (Join-Path $currentVersionPath 'relayd.exe') -DataRoot $DataRoot
if ($PSBoundParameters.ContainsKey('ObservedStorageSchema') -and
    $ObservedStorageSchema -ne $observedStorageSchema) {
    throw 'ROLLBACK_STORAGE_SCHEMA_MISMATCH'
}
if ($observedStorageSchema -eq 0) { throw 'ROLLBACK_STORAGE_SCHEMA_UNKNOWN' }
if ($observedStorageSchema -lt [int]$receipt.storage_schema_min -or
    $observedStorageSchema -gt [int]$receipt.storage_schema_max) {
    throw 'ROLLBACK_STORAGE_SCHEMA_INCOMPATIBLE'
}

Write-JsonAtomic $currentPath ([ordered]@{
    schema_version = 1
    active_version = $Version
    archive_sha256 = $ExpectedArchiveSha256.ToLowerInvariant()
    storage_schema_min = [int]$receipt.storage_schema_min
    storage_schema_max = [int]$receipt.storage_schema_max
    observed_storage_schema = $observedStorageSchema
    channel = 'unsigned_local_development'
    previous_version = [string]$current.active_version
})
Write-Output "Rolled back local development activation to version: $Version"
Write-Output 'Separately owned RELAY data and project files were not touched.'
