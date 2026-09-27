param(
    [string]$InstallRoot,
    [switch]$AllowUnsignedLocalDevelopment,
    [switch]$FixtureMode,
    [ValidatePattern('^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$')]
    [string]$Version,
    [switch]$AllVersions
)

$ErrorActionPreference = 'Stop'
if (-not $AllowUnsignedLocalDevelopment) {
    throw 'Local-development uninstall requires -AllowUnsignedLocalDevelopment'
}
if (($AllVersions -and $Version) -or (-not $AllVersions -and -not $Version)) {
    throw 'Select one version or -AllVersions'
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
    if ([IO.Path]::GetDirectoryName($full) -ne $base) { throw 'Deletion path is outside the expected parent' }
}

Assert-NoReparseAncestors $root
if (@(Get-Process -Name relayd -ErrorAction SilentlyContinue).Count -gt 0) {
    throw 'relayd.exe is running; uninstall must wait until it stops'
}
Assert-Directory $root
$markerPath = Join-Path $root 'install-root.json'
Assert-RegularFile $markerPath
$marker = Get-Content -LiteralPath $markerPath -Raw -ErrorAction Stop | ConvertFrom-Json
if ($marker.schema_version -ne 1 -or $marker.owner -ne 'relay_local_development') {
    throw 'Install root marker is invalid'
}
$versionsRoot = Join-Path $root 'versions'
$receiptsRoot = Join-Path $root 'receipts'
Assert-Directory $versionsRoot
Assert-Directory $receiptsRoot
$currentPath = Join-Path $root 'current.json'
$activeVersion = $null
$previousVersion = $null
if (Test-Path -LiteralPath $currentPath) {
    Assert-RegularFile $currentPath
    $current = Get-Content -LiteralPath $currentPath -Raw | ConvertFrom-Json
    if ($current.schema_version -ne 1 -or $current.channel -ne 'unsigned_local_development' -or
        [string]$current.active_version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$' -or
        [string]$current.archive_sha256 -notmatch '^[0-9a-f]{64}$' -or
        ($current.previous_version -and
            [string]$current.previous_version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$')) {
        throw 'Current version pointer is invalid'
    }
    $activeVersion = [string]$current.active_version
    $previousVersion = [string]$current.previous_version
}
if (-not $AllVersions -and $Version -eq $activeVersion) {
    throw 'Cannot remove the active version without -AllVersions'
}
if (-not $AllVersions -and $Version -eq $previousVersion) {
    throw 'Cannot remove the rollback version while current.json references it'
}

$targets = if ($AllVersions) {
    @(Get-ChildItem -LiteralPath $receiptsRoot -File | Where-Object { $_.Name -match '\.json$' } |
        ForEach-Object { $_.BaseName })
}
else { @($Version) }
if ($AllVersions) {
    $versionNames = @(Get-ChildItem -LiteralPath $versionsRoot -Directory | ForEach-Object { $_.Name } | Sort-Object)
    $receiptNames = @($targets | Sort-Object)
    if (($versionNames -join '|') -ne ($receiptNames -join '|')) {
        throw 'Unreceipted or missing version directory blocks uninstall'
    }
}

foreach ($targetVersion in $targets) {
    if ($targetVersion -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$') {
        throw 'Unsafe version name in install inventory'
    }
    $versionPath = Join-Path $versionsRoot $targetVersion
    $receiptPath = Join-Path $receiptsRoot "$targetVersion.json"
    Assert-DirectChild $versionPath $versionsRoot
    Assert-DirectChild $receiptPath $receiptsRoot
    Assert-Directory $versionPath
    Assert-RegularFile $receiptPath
    $receipt = Get-Content -LiteralPath $receiptPath -Raw -ErrorAction Stop | ConvertFrom-Json
    if ($receipt.schema_version -ne 1 -or $receipt.version -ne $targetVersion -or
        $receipt.channel -ne 'unsigned_local_development' -or
        [string]$receipt.archive_sha256 -notmatch '^[0-9a-f]{64}$') {
        throw 'Version receipt is invalid'
    }
    & (Join-Path $PSScriptRoot 'Verify-UnsignedStage.ps1') -PackageDirectory $versionPath
    $manifestPath = Join-Path $versionPath 'bundle-manifest.json'
    Assert-RegularFile $manifestPath
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.version -ne $targetVersion -or
        [int]$manifest.storage_schema_min -ne [int]$receipt.storage_schema_min -or
        [int]$manifest.storage_schema_max -ne [int]$receipt.storage_schema_max -or
        ($receipt.PSObject.Properties.Name -contains 'manifest_sha256' -and
            [string]$receipt.manifest_sha256 -ne (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash.ToLowerInvariant())) {
        throw 'Version manifest does not match its receipt'
    }
    foreach ($item in Get-ChildItem -LiteralPath $versionPath -Force -Recurse) {
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw 'Version contains a reparse point; uninstall requires manual review'
        }
    }
}

if ($AllVersions -and (Test-Path -LiteralPath $currentPath)) {
    Remove-Item -LiteralPath $currentPath -Force
}
foreach ($targetVersion in $targets) {
    $versionPath = Join-Path $versionsRoot $targetVersion
    $receiptPath = Join-Path $receiptsRoot "$targetVersion.json"
    Assert-DirectChild $versionPath $versionsRoot
    Assert-DirectChild $receiptPath $receiptsRoot
    Remove-Item -LiteralPath $versionPath -Recurse -Force
    Remove-Item -LiteralPath $receiptPath -Force
}
if ($AllVersions) {
    $backup = "$currentPath.bak"
    if (Test-Path -LiteralPath $backup -PathType Leaf) {
        Assert-RegularFile $backup
        Remove-Item -LiteralPath $backup -Force
    }
    if (@(Get-ChildItem -LiteralPath $versionsRoot -Force).Count -ne 0 -or
        @(Get-ChildItem -LiteralPath $receiptsRoot -Force).Count -ne 0) {
        throw 'Install inventory was not empty after uninstall'
    }
    Remove-Item -LiteralPath $versionsRoot
    Remove-Item -LiteralPath $receiptsRoot
    Remove-Item -LiteralPath $markerPath -Force
    if (@(Get-ChildItem -LiteralPath $root -Force).Count -eq 0) {
        Remove-Item -LiteralPath $root
    }
}
Write-Output "Removed local development program version(s): $($targets -join ', ')"
Write-Output 'Separately owned RELAY data and project files were not touched.'
