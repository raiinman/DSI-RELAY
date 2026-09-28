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
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $PSScriptRoot 'out' }
$outputRoot = [IO.Path]::GetFullPath($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory))
$stageName = "relay-$Version-windows-x64-unsigned-stage"
$name = "relay-$Version-windows-x64-local-test-installer"
$folder = Join-Path $outputRoot $name
$archive = "$folder.zip"
if ((Test-Path -LiteralPath $folder) -or (Test-Path -LiteralPath $archive)) {
    throw 'Local test installer for this version already exists.'
}

& (Join-Path $PSScriptRoot 'Build-UnsignedStage.ps1') `
    -Version $Version -BinaryDirectory $BinaryDirectory `
    -MinimumStorageSchema $MinimumStorageSchema `
    -MaximumStorageSchema $MaximumStorageSchema `
    -OutputDirectory $outputRoot

$innerArchive = Join-Path $outputRoot "$stageName.zip"
$innerDigest = "$innerArchive.sha256"
if (-not (Test-Path -LiteralPath $innerArchive -PathType Leaf) -or
    -not (Test-Path -LiteralPath $innerDigest -PathType Leaf)) {
    throw 'Unsigned stage did not produce an archive and digest.'
}
$null = New-Item -ItemType Directory -Path $folder
foreach ($source in @(
    $innerArchive,
    $innerDigest,
    (Join-Path $PSScriptRoot 'Install-RELAY.cmd'),
    (Join-Path $PSScriptRoot 'Install-RELAY.ps1'),
    (Join-Path $PSScriptRoot 'Install-LocalStage.ps1'),
    (Join-Path $PSScriptRoot 'Read-StorageSchema.ps1'),
    (Join-Path $PSScriptRoot 'Verify-UnsignedStage.ps1'),
    (Join-Path $PSScriptRoot 'Test-LocalActivationHealth.ps1'),
    (Join-Path $PSScriptRoot 'Uninstall-RELAY.cmd'),
    (Join-Path $PSScriptRoot 'Uninstall-RELAY.ps1'),
    (Join-Path $PSScriptRoot 'Uninstall-LocalStage.ps1')
)) {
    $item = Get-Item -LiteralPath $source -Force -ErrorAction Stop
    if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Installer input is not a regular file.'
    }
    Copy-Item -LiteralPath $source -Destination (Join-Path $folder $item.Name)
}
$readme = @"
DSI RELAY $Version - local Windows test installer

Created by RAiiNMAN. First-party RELAY code is MIT licensed.

1. Extract this whole ZIP to a folder.
2. Double-click Install-RELAY.cmd.
3. Open DSI RELAY from the Windows Start menu.
4. The app opens its local workspace view in your browser.

Optional: to connect normal ChatGPT desktop Chat, install the local plugin
in the installed program's plugins/dsi-relay-chat folder through a local
ChatGPT plugin marketplace. Start a new Chat after enabling it. This local
plugin does not work in ChatGPT web or mobile.

The installer keeps the program in your per-user Programs folder. RELAY data
and projects are kept separately. To remove the program, close RELAY and
double-click Uninstall-RELAY.cmd from this extracted folder. It asks the
installed RELAY engine to stop, then removes the program while keeping your
RELAY data and project files.

This is an unsigned local test build. Windows may show an unknown publisher
warning. It is not a signed public release and has no automatic uploads.
"@
[IO.File]::WriteAllText((Join-Path $folder 'START-HERE.txt'), ($readme + "`n"), (New-Object Text.UTF8Encoding($false)))

Add-Type -AssemblyName System.IO.Compression
$stream = [IO.File]::Open($archive, [IO.FileMode]::CreateNew)
try {
    $zip = New-Object IO.Compression.ZipArchive($stream, [IO.Compression.ZipArchiveMode]::Create, $false)
    try {
        foreach ($file in @(Get-ChildItem -LiteralPath $folder -File | Sort-Object Name)) {
            $entry = $zip.CreateEntry("$name/$($file.Name)", [IO.Compression.CompressionLevel]::Optimal)
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
Write-Output "Local test installer: $archive"
Write-Output "Installer SHA-256: $((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant())"
