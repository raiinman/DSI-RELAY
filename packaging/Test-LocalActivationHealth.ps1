param(
    [Parameter(Mandatory = $true)]
    [string]$RelaydPath,
    [Parameter(Mandatory = $true)]
    [string]$RelayPath,
    [Parameter(Mandatory = $true)]
    [string]$DataRoot,
    [Parameter(Mandatory = $true)]
    [string]$ExpectedVersion
)

$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSVersion.Major -lt 7) {
    throw 'Fixture activation health requires PowerShell 7 or newer'
}

function Assert-NoReparseAncestors([string]$Path) {
    $cursor = [IO.Path]::GetFullPath($Path)
    while ($cursor) {
        if (Test-Path -LiteralPath $cursor) {
            if ((Get-Item -LiteralPath $cursor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw 'Fixture health path contains a reparse point'
            }
        }
        $parent = [IO.Path]::GetDirectoryName($cursor)
        if (-not $parent -or $parent -eq $cursor) { break }
        $cursor = $parent
    }
}

function Invoke-HealthCommand([string]$Executable, [string]$Command, [string]$StateRoot) {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $Executable
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.Environment['RELAY_STATE_DIR'] = $StateRoot
    $start.ArgumentList.Add($Command)
    $start.ArgumentList.Add('--json')
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    $started = $false
    try {
        if (-not $process.Start()) { throw 'Fixture health command could not start' }
        $started = $true
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(8000)) {
            $process.Kill($true)
            throw 'Fixture health command timed out'
        }
        $stdout = $stdoutTask.GetAwaiter().GetResult()
        $stderr = $stderrTask.GetAwaiter().GetResult()
        if ($process.ExitCode -ne 0 -or $stderr.Length -ne 0 -or $stdout.Length -gt 65536) {
            throw "Fixture $Command command failed or exceeded its response bound (exit=$($process.ExitCode), stderr_chars=$($stderr.Length), stdout_chars=$($stdout.Length))"
        }
        $response = $stdout | ConvertFrom-Json -ErrorAction Stop
        if ($response.ok -ne $true -or $null -eq $response.result) {
            throw 'Fixture health command returned an invalid response'
        }
        return $response.result
    }
    finally {
        if ($started -and -not $process.HasExited) { $process.Kill($true) }
        $process.Dispose()
    }
}

$daemonBinary = [IO.Path]::GetFullPath($RelaydPath)
$cliBinary = [IO.Path]::GetFullPath($RelayPath)
$data = [IO.Path]::GetFullPath($DataRoot)
foreach ($binaryPath in @($daemonBinary, $cliBinary)) {
    Assert-NoReparseAncestors $binaryPath
    $binary = Get-Item -LiteralPath $binaryPath -Force -ErrorAction Stop
    if ($binary.PSIsContainer -or ($binary.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Fixture health executable is not a regular file'
    }
}
if ($data -eq [IO.Path]::GetPathRoot($data) -or $data.Length -lt 12) {
    throw 'Fixture health data root is too broad'
}
Assert-NoReparseAncestors $data
$dataItem = Get-Item -LiteralPath $data -Force -ErrorAction Stop
if (-not $dataItem.PSIsContainer -or ($dataItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'Fixture health data root is not a regular directory'
}

$start = [Diagnostics.ProcessStartInfo]::new()
$start.FileName = $daemonBinary
$start.UseShellExecute = $false
$start.CreateNoWindow = $true
$start.RedirectStandardOutput = $true
$start.RedirectStandardError = $true
$start.Environment['RELAY_STATE_DIR'] = $data
$start.Environment['RELAY_INSTANCE'] = 'fixture-' + [Guid]::NewGuid().ToString('N').Substring(0, 16)
$null = $start.Environment.Remove('RELAY_TEST_DISABLE_WATCHER')
$daemon = [Diagnostics.Process]::new()
$daemon.StartInfo = $start
$started = $false
$graceful = $false
try {
    if (-not $daemon.Start()) { throw 'Fixture daemon could not start' }
    $started = $true
    $stdoutTask = $daemon.StandardOutput.ReadToEndAsync()
    $stderrTask = $daemon.StandardError.ReadToEndAsync()
    $deadline = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() + 12000
    $ready = $false
    while ([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() -lt $deadline -and -not $daemon.HasExited) {
        $hostPath = Join-Path $data 'host.json'
        if (Test-Path -LiteralPath $hostPath -PathType Leaf) {
            try {
                $hostState = Get-Content -LiteralPath $hostPath -Raw -Encoding UTF8 | ConvertFrom-Json
                if ($hostState.pid -eq $daemon.Id -and $hostState.version -eq $ExpectedVersion) {
                    $ready = $true
                    break
                }
            }
            catch { }
        }
        Start-Sleep -Milliseconds 50
    }
    if (-not $ready) { throw 'Fixture daemon did not become ready within 12 seconds' }

    $status = Invoke-HealthCommand $cliBinary 'status' $data
    $doctor = Invoke-HealthCommand $cliBinary 'doctor' $data
    $diagnostics = Invoke-HealthCommand $cliBinary 'diagnostics' $data
    if ($daemon.HasExited -or $status.pid -ne $daemon.Id -or
        $status.version -ne $ExpectedVersion -or
        $status.recovery_state -ne 'Healthy' -or
        $doctor.healthy -ne $true -or $diagnostics.available -ne $true) {
        throw 'Fixture daemon health checks failed'
    }

    $null = Invoke-HealthCommand $cliBinary 'shutdown' $data
    $graceful = $daemon.WaitForExit(5000)
    if (-not $graceful) { throw 'Fixture daemon did not stop cleanly' }
    $stdout = $stdoutTask.GetAwaiter().GetResult()
    $stderr = $stderrTask.GetAwaiter().GetResult()
    if ($daemon.ExitCode -ne 0 -or $stdout.Length -gt 65536 -or $stderr.Length -ne 0) {
        throw 'Fixture daemon exit was not clean'
    }
    Write-Output 'Fixture daemon launched, passed status/doctor/diagnostics checks, and stopped.'
}
finally {
    if ($started -and -not $daemon.HasExited) {
        $daemon.Kill($true)
        if (-not $daemon.WaitForExit(5000)) {
            throw 'Fixture daemon could not be stopped; activation pointer requires manual recovery'
        }
    }
    $daemon.Dispose()
}
