param(
    [Parameter(Mandatory = $true)]
    [string]$ArchivePath,
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-fA-F]{64}$')]
    [string]$ExpectedArchiveSha256,
    [string]$InstallRoot,
    [switch]$AllowUnsignedLocalDevelopment,
    [switch]$FixtureMode,
    [switch]$Activate,
    [ValidateRange(0, 10000)]
    [int]$ObservedStorageSchema
)

$ErrorActionPreference = 'Stop'
if (-not $AllowUnsignedLocalDevelopment) {
    throw 'Unsigned package installation requires -AllowUnsignedLocalDevelopment'
}
if ($Activate -and -not $PSBoundParameters.ContainsKey('ObservedStorageSchema')) {
    throw 'Activation requires an observed storage schema; use 0 only when data is absent'
}
if (-not $InstallRoot) {
    $InstallRoot = Join-Path $env:LOCALAPPDATA 'Programs\DSI-RELAY'
}
$root = [IO.Path]::GetFullPath($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($InstallRoot))
$defaultRoot = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Programs\DSI-RELAY'))
if (-not $FixtureMode -and $root -ne $defaultRoot) {
    throw 'Custom install roots require -FixtureMode'
}
if ($root -eq [IO.Path]::GetPathRoot($root) -or $root.Length -lt 12) {
    throw 'Install root is too broad'
}
$archive = [IO.Path]::GetFullPath($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($ArchivePath))
$archiveItem = Get-Item -LiteralPath $archive -Force -ErrorAction Stop
if ($archiveItem.PSIsContainer -or ($archiveItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'Archive must be a regular file'
}
$archiveName = [IO.Path]::GetFileNameWithoutExtension($archive)
if ($archiveName -notmatch '^relay-([0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?)-windows-x64-unsigned-stage$') {
    throw 'Archive name is not a RELAY unsigned stage'
}
$version = $Matches[1]
$actualHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualHash -ne $ExpectedArchiveSha256.ToLowerInvariant()) {
    throw 'Archive digest does not match the expected SHA-256'
}

function Assert-Directory([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Install path is not a regular directory'
    }
}

function Assert-Within([string]$Candidate, [string]$Parent) {
    $full = [IO.Path]::GetFullPath($Candidate)
    $base = [IO.Path]::GetFullPath($Parent)
    if (-not $full.StartsWith($base + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Install path escapes its verified root'
    }
}

function Write-JsonAtomic([string]$Path, $Value) {
    $temporary = "$Path.new-$([Guid]::NewGuid().ToString('N'))"
    foreach ($existing in @($Path, "$Path.bak")) {
        if (Test-Path -LiteralPath $existing) {
            $item = Get-Item -LiteralPath $existing -Force
            if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
                throw 'Install pointer is not a regular file'
            }
        }
    }
    $bytes = (New-Object Text.UTF8Encoding($false)).GetBytes(($Value | ConvertTo-Json -Depth 6) + "`n")
    $stream = [IO.File]::Open($temporary, [IO.FileMode]::CreateNew)
    try {
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
    }
    finally { $stream.Dispose() }
    if (Test-Path -LiteralPath $Path) {
        [IO.File]::Replace($temporary, $Path, "$Path.bak")
    }
    else { [IO.File]::Move($temporary, $Path) }
}

function Assert-DaemonStopped {
    if (@(Get-Process -Name relayd -ErrorAction SilentlyContinue).Count -gt 0) {
        throw 'relayd.exe is running; activation or uninstall must wait until it stops'
    }
}

if (Test-Path -LiteralPath $root) {
    Assert-Directory $root
    $markerPath = Join-Path $root 'install-root.json'
    if (-not (Test-Path -LiteralPath $markerPath -PathType Leaf)) {
        throw 'Existing install root is not owned by the RELAY local installer'
    }
    $marker = Get-Content -LiteralPath $markerPath -Raw | ConvertFrom-Json
    if ($marker.schema_version -ne 1 -or $marker.owner -ne 'relay_local_development') {
        throw 'Install root marker is invalid'
    }
}
else {
    $null = New-Item -ItemType Directory -Path $root
    Assert-Directory $root
    Write-JsonAtomic (Join-Path $root 'install-root.json') ([ordered]@{
        schema_version = 1
        owner = 'relay_local_development'
    })
}
$versionsRoot = Join-Path $root 'versions'
$receiptsRoot = Join-Path $root 'receipts'
foreach ($directory in @($versionsRoot, $receiptsRoot)) {
    if (-not (Test-Path -LiteralPath $directory)) {
        $null = New-Item -ItemType Directory -Path $directory
    }
    Assert-Directory $directory
}
$versionPath = Join-Path $versionsRoot $version
$receiptPath = Join-Path $receiptsRoot "$version.json"
Assert-Within $versionPath $versionsRoot
Assert-Within $receiptPath $receiptsRoot

if (-not (Test-Path -LiteralPath $versionPath)) {
    if (Test-Path -LiteralPath $receiptPath) {
        throw 'Receipt exists without its version directory'
    }
    Add-Type -AssemblyName System.IO.Compression
    $stagePath = Join-Path $versionsRoot ('.stage-' + [Guid]::NewGuid().ToString('N'))
    Assert-Within $stagePath $versionsRoot
    $null = New-Item -ItemType Directory -Path $stagePath
    $stream = [IO.File]::OpenRead($archive)
    try {
        $zip = New-Object IO.Compression.ZipArchive($stream, [IO.Compression.ZipArchiveMode]::Read, $false)
        try {
            if ($zip.Entries.Count -gt 512) { throw 'Archive contains too many files' }
            $seen = @{}
            $totalBytes = [int64]0
            $prefix = "$archiveName/"
            foreach ($entry in $zip.Entries) {
                $path = [string]$entry.FullName
                if (-not $path.StartsWith($prefix, [StringComparison]::Ordinal) -or
                    $path.EndsWith('/') -or $path.Length -gt 256) {
                    throw 'Archive contains an unexpected entry'
                }
                $relative = $path.Substring($prefix.Length)
                $segments = $relative.Split('/')
                if ($relative -notmatch '^[A-Za-z0-9._/-]+$' -or
                    @($segments | Where-Object { $_ -eq '' -or $_ -eq '.' -or $_ -eq '..' }).Count -gt 0 -or
                    $seen.ContainsKey($relative.ToLowerInvariant())) {
                    throw 'Archive contains an unsafe or duplicate path'
                }
                $seen[$relative.ToLowerInvariant()] = $true
                if ((($entry.ExternalAttributes -shr 16) -band 0xf000) -eq 0xa000) {
                    throw 'Archive contains a symlink'
                }
                $totalBytes += [int64]$entry.Length
                if ($totalBytes -gt 256MB) { throw 'Archive expansion exceeds limit' }
                $destination = Join-Path $stagePath ($relative.Replace('/', '\'))
                Assert-Within $destination $stagePath
                $parent = Split-Path $destination -Parent
                $null = New-Item -ItemType Directory -Path $parent -Force
                $input = $entry.Open()
                try {
                    $output = [IO.File]::Open($destination, [IO.FileMode]::CreateNew)
                    try { $input.CopyTo($output) }
                    finally { $output.Dispose() }
                }
                finally { $input.Dispose() }
            }
        }
        finally { $zip.Dispose() }
    }
    finally { $stream.Dispose() }
    & (Join-Path $PSScriptRoot 'Verify-UnsignedStage.ps1') -PackageDirectory $stagePath
    $manifest = Get-Content -LiteralPath (Join-Path $stagePath 'bundle-manifest.json') -Raw | ConvertFrom-Json
    if ($manifest.version -ne $version -or
        [int]$manifest.storage_schema_min -lt 1 -or
        [int]$manifest.storage_schema_max -lt [int]$manifest.storage_schema_min) {
        throw 'Archive manifest version or schema range is invalid'
    }
    if (Test-Path -LiteralPath $versionPath) { throw 'Version appeared during staging' }
    Move-Item -LiteralPath $stagePath -Destination $versionPath
    Write-JsonAtomic $receiptPath ([ordered]@{
        schema_version = 1
        version = $version
        archive_sha256 = $actualHash
        storage_schema_min = [int]$manifest.storage_schema_min
        storage_schema_max = [int]$manifest.storage_schema_max
        channel = 'unsigned_local_development'
    })
}
else {
    Assert-Directory $versionPath
    if (-not (Test-Path -LiteralPath $receiptPath -PathType Leaf)) {
        throw 'Existing version lacks an install receipt'
    }
    & (Join-Path $PSScriptRoot 'Verify-UnsignedStage.ps1') -PackageDirectory $versionPath
}
$receipt = Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json
if ($receipt.schema_version -ne 1 -or $receipt.version -ne $version -or
    $receipt.archive_sha256 -ne $actualHash -or
    $receipt.channel -ne 'unsigned_local_development') {
    throw 'Existing version receipt does not match the archive'
}

if ($Activate) {
    Assert-DaemonStopped
    if ($ObservedStorageSchema -ne 0 -and
        ($ObservedStorageSchema -lt [int]$receipt.storage_schema_min -or
            $ObservedStorageSchema -gt [int]$receipt.storage_schema_max)) {
        throw 'UPDATE_STORAGE_SCHEMA_INCOMPATIBLE'
    }
    $currentPath = Join-Path $root 'current.json'
    $previousVersion = $null
    if (Test-Path -LiteralPath $currentPath -PathType Leaf) {
        $previous = Get-Content -LiteralPath $currentPath -Raw | ConvertFrom-Json
        if ($previous.schema_version -ne 1 -or $previous.channel -ne 'unsigned_local_development' -or
            [string]$previous.active_version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$') {
            throw 'Current version pointer is invalid'
        }
        $previousVersion = [string]$previous.active_version
        $previousPath = Join-Path $versionsRoot $previousVersion
        $previousReceiptPath = Join-Path $receiptsRoot "$previousVersion.json"
        Assert-Within $previousPath $versionsRoot
        Assert-Within $previousReceiptPath $receiptsRoot
        Assert-Directory $previousPath
        $previousReceipt = Get-Content -LiteralPath $previousReceiptPath -Raw -ErrorAction Stop | ConvertFrom-Json
        if ($previousReceipt.version -ne $previousVersion -or
            $previousReceipt.archive_sha256 -ne $previous.archive_sha256) {
            throw 'Current version receipt is inconsistent'
        }
        & (Join-Path $PSScriptRoot 'Verify-UnsignedStage.ps1') -PackageDirectory $previousPath
    }
    Write-JsonAtomic $currentPath ([ordered]@{
        schema_version = 1
        active_version = $version
        archive_sha256 = $actualHash
        storage_schema_min = [int]$receipt.storage_schema_min
        storage_schema_max = [int]$receipt.storage_schema_max
        observed_storage_schema = $ObservedStorageSchema
        channel = 'unsigned_local_development'
        previous_version = $previousVersion
    })
    Write-Output "Activated local development version: $version"
}
else { Write-Output "Staged local development version without activation: $version" }
