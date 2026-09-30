param(
    [Parameter(Mandatory = $true)]
    [string]$ArchivePath,
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9a-fA-F]{64}$')]
    [string]$ExpectedArchiveSha256,
    [string]$InstallRoot,
    [string]$DataRoot,
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
if ($DataRoot -and -not $FixtureMode) { throw 'Custom data roots require -FixtureMode' }
if ($FixtureMode -and $Activate -and -not $DataRoot) {
    throw 'Fixture activation requires an explicit fixture data root'
}
if (-not $FixtureMode -and $Activate -and $env:RELAY_STATE_DIR) {
    throw 'Local activation cannot infer the data root while RELAY_STATE_DIR is set'
}
if (-not $DataRoot) {
    if (-not $env:LOCALAPPDATA) { throw 'LOCALAPPDATA is required for local activation' }
    $DataRoot = Join-Path $env:LOCALAPPDATA 'DSI\RELAY'
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
Assert-NoReparseAncestors $root
Write-Verbose 'RELAY install: install-root ancestry verified'
$archive = [IO.Path]::GetFullPath($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($ArchivePath))
$archiveItem = Get-Item -LiteralPath $archive -Force -ErrorAction Stop
if ($archiveItem.PSIsContainer -or ($archiveItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'Archive must be a regular file'
}
$archiveName = [IO.Path]::GetFileNameWithoutExtension($archive)
if (-not ($archiveName -match '^relay-([0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?)-windows-x64-unsigned-stage$')) {
    throw 'Archive name is not a RELAY unsigned stage'
}
$version = $Matches[1]
$actualHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualHash -ne $ExpectedArchiveSha256.ToLowerInvariant()) {
    throw 'Archive digest does not match the expected SHA-256'
}
Write-Verbose 'RELAY install: archive digest verified'

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
    $versionPrefix = [IO.Path]::GetFullPath((Join-Path $root 'versions')) + [IO.Path]::DirectorySeparatorChar
    foreach ($daemon in @(Get-Process -Name relayd -ErrorAction SilentlyContinue)) {
        if ($daemon.Path -and $daemon.Path.StartsWith($versionPrefix, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'An installed RELAY engine is running; activation must wait until it stops'
        }
    }
    $hostPath = Join-Path $DataRoot 'host.json'
    if (Test-Path -LiteralPath $hostPath) {
        Assert-RegularFile $hostPath
        $hostState = Get-Content -LiteralPath $hostPath -Raw -ErrorAction Stop | ConvertFrom-Json
        if ([string]$hostState.pid -match '^[1-9][0-9]*$') {
            $selectedDaemon = Get-Process -Id ([int]$hostState.pid) -ErrorAction SilentlyContinue
            if ($selectedDaemon -and $selectedDaemon.ProcessName -eq 'relayd') {
                throw 'The selected RELAY data root has a running engine; activation must wait until it stops'
            }
        }
    }
}

if (Test-Path -LiteralPath $root) {
    Assert-Directory $root
    $markerPath = Join-Path $root 'install-root.json'
    if (-not (Test-Path -LiteralPath $markerPath -PathType Leaf)) {
        throw 'Existing install root is not owned by the RELAY local installer'
    }
    Assert-RegularFile $markerPath
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
        manifest_sha256 = (Get-FileHash -LiteralPath (Join-Path $versionPath 'bundle-manifest.json') -Algorithm SHA256).Hash.ToLowerInvariant()
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
Assert-RegularFile $receiptPath
$receipt = Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json
if ($receipt.schema_version -ne 1 -or $receipt.version -ne $version -or
    $receipt.archive_sha256 -ne $actualHash -or
    $receipt.channel -ne 'unsigned_local_development') {
    throw 'Existing version receipt does not match the archive'
}
$manifestPath = Join-Path $versionPath 'bundle-manifest.json'
Assert-RegularFile $manifestPath
$installedManifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ($installedManifest.version -ne $version -or
    [int]$receipt.storage_schema_min -lt 1 -or
    [int]$receipt.storage_schema_max -lt [int]$receipt.storage_schema_min -or
    [int]$installedManifest.storage_schema_min -ne [int]$receipt.storage_schema_min -or
    [int]$installedManifest.storage_schema_max -ne [int]$receipt.storage_schema_max) {
    throw 'Installed manifest does not match its receipt'
}
if ($receipt.PSObject.Properties.Name -contains 'manifest_sha256' -and
    [string]$receipt.manifest_sha256 -ne (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash.ToLowerInvariant()) {
    throw 'Installed manifest digest does not match its receipt'
}
Write-Verbose 'RELAY install: target package and receipt verified'

if ($Activate) {
    Assert-DaemonStopped
    $currentPath = Join-Path $root 'current.json'
    $previousVersion = $null
    if (Test-Path -LiteralPath $currentPath) {
        Assert-RegularFile $currentPath
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
        Assert-RegularFile $previousReceiptPath
        $previousReceipt = Get-Content -LiteralPath $previousReceiptPath -Raw -ErrorAction Stop | ConvertFrom-Json
        if ($previousReceipt.version -ne $previousVersion -or
            $previousReceipt.archive_sha256 -ne $previous.archive_sha256 -or
            [int]$previousReceipt.storage_schema_min -ne [int]$previous.storage_schema_min -or
            [int]$previousReceipt.storage_schema_max -ne [int]$previous.storage_schema_max) {
            throw 'Current version receipt is inconsistent'
        }
        & (Join-Path $PSScriptRoot 'Verify-UnsignedStage.ps1') -PackageDirectory $previousPath
        $previousManifest = Get-Content -LiteralPath (Join-Path $previousPath 'bundle-manifest.json') -Raw | ConvertFrom-Json
        if ($previousManifest.version -ne $previousVersion -or
            [int]$previousManifest.storage_schema_min -ne [int]$previousReceipt.storage_schema_min -or
            [int]$previousManifest.storage_schema_max -ne [int]$previousReceipt.storage_schema_max) {
            throw 'Current version manifest does not match its receipt'
        }
        if ($previousReceipt.PSObject.Properties.Name -contains 'manifest_sha256' -and
            [string]$previousReceipt.manifest_sha256 -ne
                (Get-FileHash -LiteralPath (Join-Path $previousPath 'bundle-manifest.json') -Algorithm SHA256).Hash.ToLowerInvariant()) {
            throw 'Current version manifest digest does not match its receipt'
        }
        Write-Verbose 'RELAY install: current active package and pointer verified'
    }
    Write-Verbose 'RELAY install: probing storage schema'
    $observedStorageSchema = & (Join-Path $PSScriptRoot 'Read-StorageSchema.ps1') `
        -RelaydPath (Join-Path $versionPath 'relayd.exe') -DataRoot $DataRoot
    if ($PSBoundParameters.ContainsKey('ObservedStorageSchema') -and
        $ObservedStorageSchema -ne $observedStorageSchema) {
        throw 'UPDATE_STORAGE_SCHEMA_MISMATCH'
    }
    if ($previousVersion -and $observedStorageSchema -eq 0) {
        throw 'UPDATE_STORAGE_SCHEMA_UNKNOWN: the existing RELAY database is absent'
    }
    if ($observedStorageSchema -ne 0 -and
        ($observedStorageSchema -lt [int]$receipt.storage_schema_min -or
            $observedStorageSchema -gt [int]$receipt.storage_schema_max)) {
        throw 'UPDATE_STORAGE_SCHEMA_INCOMPATIBLE'
    }
    Write-Verbose "RELAY install: storage schema $observedStorageSchema accepted"
    if ($previousVersion -eq $version) {
        # Re-running the same verified installer is a repair/no-op, not a
        # side-by-side update. Keep rollback history from pointing at itself.
        if ($previous.PSObject.Properties.Name -contains 'observed_storage_schema') {
            $previous.observed_storage_schema = $observedStorageSchema
        }
        else {
            $previous | Add-Member -NotePropertyName observed_storage_schema -NotePropertyValue $observedStorageSchema
        }
        if ($previous.PSObject.Properties.Name -contains 'previous_version' -and
            [string]$previous.previous_version -eq $version) {
            $previous.previous_version = $null
        }
        Write-JsonAtomic $currentPath $previous
        Write-Output "Already active local development version: $version"
        return
    }
    if ($FixtureMode -and $previousVersion -and
        [int]$receipt.storage_schema_max -gt [int]$previousReceipt.storage_schema_max) {
        throw 'UPDATE_AUTO_ROLLBACK_SCHEMA_RANGE_INCOMPATIBLE'
    }
    $activationPointer = [ordered]@{
        schema_version = 1
        active_version = $version
        archive_sha256 = $actualHash
        storage_schema_min = [int]$receipt.storage_schema_min
        storage_schema_max = [int]$receipt.storage_schema_max
        observed_storage_schema = $observedStorageSchema
        channel = 'unsigned_local_development'
        previous_version = $previousVersion
    }
    Write-JsonAtomic $currentPath $activationPointer
    if ($FixtureMode) {
        try {
            $null = & (Join-Path $PSScriptRoot 'Test-LocalActivationHealth.ps1') `
                -RelaydPath (Join-Path $versionPath 'relayd.exe') `
                -RelayPath (Join-Path $versionPath 'relay.exe') `
                -DataRoot $DataRoot -ExpectedVersion $version
            Assert-DaemonStopped
            $postLaunchSchema = & (Join-Path $PSScriptRoot 'Read-StorageSchema.ps1') `
                -RelaydPath (Join-Path $versionPath 'relayd.exe') -DataRoot $DataRoot
            if ($postLaunchSchema -lt [int]$receipt.storage_schema_min -or
                $postLaunchSchema -gt [int]$receipt.storage_schema_max) {
                throw 'UPDATE_POST_LAUNCH_SCHEMA_INCOMPATIBLE'
            }
            if ($previousVersion -and
                ($postLaunchSchema -lt [int]$previousReceipt.storage_schema_min -or
                    $postLaunchSchema -gt [int]$previousReceipt.storage_schema_max)) {
                throw 'UPDATE_POST_LAUNCH_ROLLBACK_SCHEMA_INCOMPATIBLE'
            }
            $activationPointer.observed_storage_schema = $postLaunchSchema
            Write-JsonAtomic $currentPath $activationPointer
        }
        catch {
            $activationError = $_.Exception.Message
            Assert-DaemonStopped
            if ($previousVersion) {
                $rollbackSchema = & (Join-Path $PSScriptRoot 'Read-StorageSchema.ps1') `
                    -RelaydPath (Join-Path $previousPath 'relayd.exe') -DataRoot $DataRoot
                if ($rollbackSchema -lt [int]$previousReceipt.storage_schema_min -or
                    $rollbackSchema -gt [int]$previousReceipt.storage_schema_max) {
                    throw 'ACTIVATION_HEALTH_FAILED_ROLLBACK_SCHEMA_INCOMPATIBLE'
                }
                if ($previous.PSObject.Properties.Name -contains 'observed_storage_schema') {
                    $previous.observed_storage_schema = $rollbackSchema
                }
                else {
                    $previous | Add-Member -NotePropertyName observed_storage_schema -NotePropertyValue $rollbackSchema
                }
                Write-JsonAtomic $currentPath $previous
                throw "ACTIVATION_HEALTH_FAILED_PREVIOUS_RESTORED: $activationError"
            }
            Assert-RegularFile $currentPath
            Remove-Item -LiteralPath $currentPath -Force
            throw "ACTIVATION_HEALTH_FAILED_NO_ACTIVE_VERSION: $activationError"
        }
    }
    Write-Output "Activated local development version: $version"
}
else { Write-Output "Staged local development version without activation: $version" }
