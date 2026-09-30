param(
    [Parameter(Mandatory = $true)]
    [string]$RelaydPath,
    [Parameter(Mandatory = $true)]
    [string]$DataRoot
)

$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSVersion.Major -lt 5) {
    throw 'Storage schema probing requires Windows PowerShell 5.1 or newer'
}

function Assert-NoReparseAncestors([string]$Path) {
    $cursor = [IO.Path]::GetFullPath($Path)
    while ($cursor) {
        if (Test-Path -LiteralPath $cursor) {
            $item = Get-Item -LiteralPath $cursor -Force
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw 'Storage probe path contains a reparse point'
            }
        }
        $parent = [IO.Path]::GetDirectoryName($cursor)
        if (-not $parent -or $parent -eq $cursor) { break }
        $cursor = $parent
    }
}

$executable = [IO.Path]::GetFullPath($RelaydPath)
$data = [IO.Path]::GetFullPath($DataRoot)
Assert-NoReparseAncestors $executable
Assert-NoReparseAncestors $data
if ($data -eq [IO.Path]::GetPathRoot($data) -or $data.Length -lt 12) {
    throw 'Storage probe data root is too broad'
}
$binary = Get-Item -LiteralPath $executable -Force -ErrorAction Stop
if ($binary.PSIsContainer -or ($binary.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'Storage probe executable is not a regular file'
}

$start = [Diagnostics.ProcessStartInfo]::new()
$start.FileName = $executable
$start.UseShellExecute = $false
$start.CreateNoWindow = $true
$start.RedirectStandardOutput = $true
$start.RedirectStandardError = $true
if ($data.Contains([char]34)) { throw 'Storage probe data root contains an invalid quote' }
# Install-RELAY.cmd intentionally runs in inbox Windows PowerShell. Use the
# ProcessStartInfo.Arguments string available there instead of the PowerShell
# 7-only ArgumentList collection. Data roots cannot contain quotes on Windows;
# trim only trailing separators so the quoted argument is not escape-ambiguous.
$dataArgument = $data.TrimEnd([char[]]@([char]92, [char]47))
$start.Arguments = '--probe-storage-schema --state-dir "' + $dataArgument + '"'
$process = [Diagnostics.Process]::new()
$process.StartInfo = $start
$started = $false
try {
    if (-not $process.Start()) { throw 'Storage schema probe could not start' }
    $started = $true
    # The probe contract emits only a tiny JSON object. Wait for the bounded
    # child first, then read the already-closed pipes synchronously. This avoids
    # PowerShell 5.1/.NET Framework async stream waits that can stall the
    # double-click installer even after the probe process has exited.
    if (-not $process.WaitForExit(30000)) {
        $process.Kill()
        throw 'Storage schema probe timed out'
    }
    $stdout = $process.StandardOutput.ReadToEnd()
    $stderr = $process.StandardError.ReadToEnd()
    if ($stdout.Length -gt 256 -or $stderr.Length -ne 0 -or $process.ExitCode -ne 0) {
        throw 'Storage schema probe failed or returned an invalid response'
    }
    $response = $stdout | ConvertFrom-Json -ErrorAction Stop
    if ($response.ok -ne $true -or
        $response.PSObject.Properties.Name -notcontains 'schema_version' -or
        $response.schema_version -isnot [long] -and $response.schema_version -isnot [int] -or
        [long]$response.schema_version -lt 0 -or [long]$response.schema_version -gt 10000 -or
        $response.database -notin @('absent', 'present') -or
        (($response.database -eq 'absent') -ne ([long]$response.schema_version -eq 0))) {
        throw 'Storage schema probe returned an invalid response'
    }
    return [int]$response.schema_version
}
finally {
    if ($started -and -not $process.HasExited) { $process.Kill() }
    $process.Dispose()
}
