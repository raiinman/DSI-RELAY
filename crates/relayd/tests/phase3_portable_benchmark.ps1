param(
    [string]$BinaryDirectory = $PSScriptRoot,
    [ValidateRange(100, 7500)]
    [int]$FilesPerProject = 1000,
    [ValidateRange(1, 20)]
    [int]$Runs = 3,
    [string]$OutputPath = (Join-Path (Get-Location) ("relay-phase3-benchmark-{0}-{1}.json" -f (Get-Date -Format 'yyyyMMdd-HHmmss'), ([Guid]::NewGuid().ToString('N').Substring(0, 8)))),
    [ValidateRange(1, 600000)]
    [int]$CommandTimeoutMs = 180000
)

$ErrorActionPreference = 'Stop'
$outputFull = [System.IO.Path]::GetFullPath($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputPath))
$binaryRoot = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($BinaryDirectory)
$daemonPath = Join-Path $binaryRoot 'relayd.exe'
$cliPath = Join-Path $binaryRoot 'relay.exe'
$tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\')
$script:reportInitialized = $false
$script:cliTimedOut = $false
$script:benchmarkFailureCode = $null
$utf8 = New-Object System.Text.UTF8Encoding($false)
$report = [ordered]@{
    schema_version = 2
    benchmark_version = '0.2.0-draft'
    captured_utc = (Get-Date).ToUniversalTime().ToString('o')
    status = 'in_progress'
    current_stage = 'preflight'
    failure_code = $null
    runs_requested = $Runs
    runs_completed = 0
    fixture = 'two-generic-watched-projects'
    file_count_per_project = $FilesPerProject
    host_class = $null
    binaries = $null
    samples = @()
    caveats = @(
        'Synthetic small text files and a warm local filesystem; this report alone is not a tier budget.',
        'Watcher wall time includes command-line process startup and polling.',
        'Five seconds of idle observation per run cannot establish long-run CPU, power, or wakeup cost.',
        'No creator application or parser worker is included.'
    )
}

function Save-Report {
    $bytes = $utf8.GetBytes(($report | ConvertTo-Json -Depth 12) + [Environment]::NewLine)
    if (-not $script:reportInitialized) {
        $parent = [System.IO.Path]::GetDirectoryName($outputFull)
        $initial = Join-Path $parent ('.relay-report-initial-' + [Guid]::NewGuid().ToString('N') + '.tmp')
        try {
            $stream = New-Object System.IO.FileStream($initial, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
            try { $stream.Write($bytes, 0, $bytes.Length) }
            finally { $stream.Dispose() }
            [System.IO.File]::Move($initial, $outputFull)
        }
        finally {
            if (Test-Path -LiteralPath $initial) { Remove-Item -LiteralPath $initial -Force }
        }
        $script:reportInitialized = $true
        return
    }
    $parent = [System.IO.Path]::GetDirectoryName($outputFull)
    $temp = Join-Path $parent ('.relay-report-' + [Guid]::NewGuid().ToString('N') + '.tmp')
    $backup = Join-Path $parent ('.relay-report-backup-' + [Guid]::NewGuid().ToString('N') + '.tmp')
    try {
        $stream = New-Object System.IO.FileStream($temp, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
        try { $stream.Write($bytes, 0, $bytes.Length) }
        finally { $stream.Dispose() }
        [System.IO.File]::Replace($temp, $outputFull, $backup)
    }
    finally {
        if (Test-Path -LiteralPath $temp) { Remove-Item -LiteralPath $temp -Force }
        if (Test-Path -LiteralPath $backup) { Remove-Item -LiteralPath $backup -Force }
    }
}

function Set-Stage {
    param([string]$Stage)
    $report.current_stage = $Stage
    Save-Report
}

Save-Report

function Invoke-Relay {
    param([string]$Command, [hashtable]$Arguments, [bool]$Keyed = $false, [int]$TimeoutMs = $CommandTimeoutMs)
    $id = 'BENCH-' + [Guid]::NewGuid().ToString('N')
    $inputObject = @{
        request_id = $id
        command = $Command
        command_version = 1
        arguments = $Arguments
    }
    if ($Keyed) { $inputObject.idempotency_key = "IDEMP-$id" }
    $inputJson = $inputObject | ConvertTo-Json -Depth 8 -Compress
    $inputBytes = $utf8.GetBytes($inputJson + [Environment]::NewLine)
    $ioRoot = Join-Path $ownedFull ('cli-' + [Guid]::NewGuid().ToString('N'))
    $inputPath = $ioRoot + '.in'
    $stdoutPath = $ioRoot + '.out'
    $stderrPath = $ioRoot + '.err'
    [System.IO.File]::WriteAllBytes($inputPath, $inputBytes)
    $child = $null
    try {
        try {
            $child = Start-Process -FilePath $cliPath -ArgumentList 'exec --stdin' -PassThru -WindowStyle Hidden `
                -RedirectStandardInput $inputPath -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath
        }
        catch {
            $script:benchmarkFailureCode = 'BENCHMARK_CLI_LAUNCH_FAILED'
            throw
        }
        if (-not $child.WaitForExit($TimeoutMs)) {
            $script:cliTimedOut = $true
            throw 'RELAY_CLI_TIMEOUT'
        }
        $responseText = [System.IO.File]::ReadAllText($stdoutPath, $utf8)
        $stderrText = [System.IO.File]::ReadAllText($stderrPath, $utf8)
        $exitCode = $child.ExitCode
        if ($null -eq $exitCode) {
            # Windows PowerShell 5.1 does not expose ExitCode from a redirected Start-Process.
            $exitCode = if ($stderrText -or -not $responseText) { 1 } else { 0 }
        }
    }
    finally {
        if ($child -and -not $child.HasExited) {
            try { $child.Kill() } catch { }
            $null = $child.WaitForExit(5000)
        }
        if ($child) { $child.Dispose() }
        foreach ($path in @($inputPath, $stdoutPath, $stderrPath)) {
            if (Test-Path -LiteralPath $path -PathType Leaf) { Remove-Item -LiteralPath $path -Force }
        }
    }
    if ($exitCode -ne 0) {
        $script:benchmarkFailureCode = 'BENCHMARK_CLI_EXIT_NONZERO'
        Write-Host "RELAY CLI $Command exited with code $exitCode"
        $prefixLength = [Math]::Min(4, $inputBytes.Length)
        $prefix = if ($prefixLength -gt 0) { [BitConverter]::ToString($inputBytes, 0, $prefixLength) } else { 'EMPTY' }
        Write-Host "RELAY CLI input length $($inputBytes.Length), first bytes $prefix"
        try {
            $cliFailure = $stderrText | ConvertFrom-Json
            $message = [string]$cliFailure.error.message
            $safeCategory = $message.Split(':')[0]
            if ($safeCategory -in @(
                'HOST_UNAVAILABLE', 'HOST_STATE_INCOMPATIBLE', 'INVALID_MACHINE_INPUT',
                'read stdin', 'parse host state', 'open local pipe failed',
                'read local pipe failed', 'write local pipe failed', 'pipe closed',
                'pipe returned zero bytes', 'pipe message too large', 'pipe utf8',
                'pipe wrote zero bytes',
                'parse response', 'parse hello response', 'parse command response',
                'serialize request', 'BAD_HANDSHAKE', 'UNAUTHORIZED',
                'PROTOCOL_INCOMPATIBLE'
            )) { Write-Host "RELAY CLI error category: $safeCategory" }
            if ($safeCategory -eq 'INVALID_MACHINE_INPUT' -and $message.Length -le 240 -and
                $message -match '^INVALID_MACHINE_INPUT: [\x20-\x7e]+$' -and
                $message -notmatch '[\\/@]' -and
                $message.IndexOf([string]$env:USERNAME, [StringComparison]::OrdinalIgnoreCase) -lt 0 -and
                $message.IndexOf([string]$env:COMPUTERNAME, [StringComparison]::OrdinalIgnoreCase) -lt 0) {
                Write-Host "RELAY CLI parser detail: $message"
            }
        }
        catch { }
        throw "RELAY command $Command failed"
    }
    $response = $responseText | ConvertFrom-Json
    if (-not $response.ok) {
        $script:benchmarkFailureCode = 'BENCHMARK_COMMAND_ERROR'
        $code = [string]$response.error.code
        if ($code -match '^[A-Z][A-Z0-9_]{0,63}$') { Write-Host "RELAY command $Command returned $code" }
        throw "RELAY command $Command returned an error"
    }
    return $response.result
}

function Wait-Daemon {
    param([System.Diagnostics.Process]$Process, [string]$StderrPath)
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    $statePath = Join-Path $stateDir 'host.json'
    while ([DateTime]::UtcNow -lt $deadline) {
        if ($Process.HasExited) {
            $script:benchmarkFailureCode = 'BENCHMARK_DAEMON_EXITED'
            if (Test-Path -LiteralPath $StderrPath -PathType Leaf) {
                try {
                    $failure = Get-Content -LiteralPath $StderrPath -Raw | ConvertFrom-Json
                    $message = [string]$failure.error.message
                    if ($message -match '^kernel verification rejected local pipe security: query_ok=(true|false), protected_dacl=(true|false), owner_is_current_user=(true|false), current_user_only=(true|false), current_user_full_control=(true|false), ace_count=[0-9]{1,3}$') {
                        $script:benchmarkFailureCode = 'BENCHMARK_DAEMON_PIPE_ACL_REJECTED'
                        Write-Host $message
                    }
                    elseif ($message -match '^(OpenProcessToken|GetTokenInformation|ConvertSidToStringSidW|ConvertStringSecurityDescriptorToSecurityDescriptorW|CreateNamedPipeW|GetSecurityInfo|ConvertSecurityDescriptorToStringSecurityDescriptorW) failed:') {
                        $script:benchmarkFailureCode = 'BENCHMARK_DAEMON_PIPE_SECURITY_FAILED'
                    }
                    elseif ($message -match '^dashboard (loopback bind|listener setup|local address) failed:') {
                        $script:benchmarkFailureCode = 'BENCHMARK_DAEMON_DASHBOARD_LISTEN_FAILED'
                    }
                    elseif ($message -match '^(create restricted dashboard state|write restricted dashboard state|publish dashboard state)') {
                        $script:benchmarkFailureCode = 'BENCHMARK_DAEMON_DASHBOARD_STATE_FAILED'
                    }
                    elseif ($message -match '^(create state dir|write host state|publish host state)') {
                        $script:benchmarkFailureCode = 'BENCHMARK_DAEMON_HOST_STATE_FAILED'
                    }
                }
                catch { }
            }
            throw 'RELAY daemon exited before becoming ready'
        }
        if (Test-Path -LiteralPath $statePath) {
            try {
                $state = Get-Content -LiteralPath $statePath -Raw | ConvertFrom-Json
                if ($state.pid -eq $Process.Id) { return $state }
            }
            catch { }
        }
        Start-Sleep -Milliseconds 50
    }
    $script:benchmarkFailureCode = 'BENCHMARK_DAEMON_NOT_READY'
    throw 'RELAY daemon did not become ready in ten seconds'
}

function New-SyntheticProject {
    param([string]$Root)
    for ($number = 0; $number -lt $FilesPerProject; $number++) {
        $group = [int][Math]::Floor($number / 100)
        $folder = Join-Path $Root ('set-{0:D2}' -f $group)
        if (-not (Test-Path -LiteralPath $folder)) { $null = New-Item -ItemType Directory -Path $folder }
        $file = Join-Path $folder ('file-{0:D4}.txt' -f $number)
        [System.IO.File]::WriteAllText($file, 'generic project fixture content for indexing')
    }
}

function Get-HostClass {
    $cpu = Get-CimInstance Win32_Processor
    $computer = Get-CimInstance Win32_ComputerSystem
    $storage = 'unknown'
    try {
        $drive = [System.IO.Path]::GetPathRoot($tempRoot).Substring(0, 1)
        $partition = Get-Partition -DriveLetter $drive -ErrorAction Stop
        $disk = Get-Disk -Number $partition.DiskNumber -ErrorAction Stop
        $media = [string]$disk.MediaType
        if ($media -notin @('SSD', 'HDD', 'SCM')) {
            $physical = Get-PhysicalDisk -ErrorAction Stop |
                Where-Object { $_.DeviceId -eq [string]$disk.Number } |
                Select-Object -First 1
            $media = [string]$physical.MediaType
        }
        if ($media -in @('SSD', 'HDD', 'SCM')) { $storage = $media.ToLowerInvariant() }
    }
    catch { }
    $powerPlan = 'unknown'
    try {
        $active = (& powercfg /getactivescheme 2>$null | Out-String)
        if ($active -match '381b4222-f694-41f0-9685-ff5bb260df2e') { $powerPlan = 'balanced' }
        elseif ($active -match '8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c') { $powerPlan = 'high_performance' }
        elseif ($active -match 'a1841308-3541-4fab-bc81-f71556f20b4a') { $powerPlan = 'power_saver' }
        elseif ($active -match '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}') { $powerPlan = 'custom' }
    }
    catch { }
    return [ordered]@{
        cpu_model = ($cpu | Select-Object -First 1).Name.Trim()
        physical_cores = [int](($cpu | Measure-Object NumberOfCores -Sum).Sum)
        logical_processors = [int](($cpu | Measure-Object NumberOfLogicalProcessors -Sum).Sum)
        ram_bytes = [uint64]$computer.TotalPhysicalMemory
        os_version = [Environment]::OSVersion.Version.ToString()
        temp_storage_class = $storage
        power_plan_class = $powerPlan
    }
}

function Remove-OwnedRoot {
    param([string]$Path)
    if (-not $Path -or -not (Test-Path -LiteralPath $Path)) { return }
    $full = [System.IO.Path]::GetFullPath($Path)
    $name = [System.IO.Path]::GetFileName($full)
    if ([System.IO.Path]::GetDirectoryName($full) -ne $tempRoot -or
        $name -notmatch '^relay-portable-benchmark-[0-9a-f]{32}$') {
        throw 'Owned temporary directory failed path validation'
    }
    $item = Get-Item -LiteralPath $full -Force
    if (-not $item.PSIsContainer -or ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint)) {
        throw 'Owned temporary directory changed into a reparse point'
    }
    $resolved = (Resolve-Path -LiteralPath $full).Path
    if ([System.IO.Path]::GetDirectoryName($resolved) -ne $tempRoot -or
        [System.IO.Path]::GetFileName($resolved) -ne $name) {
        throw 'Owned temporary directory resolved outside the temporary root'
    }
    $pending = New-Object 'System.Collections.Generic.Stack[string]'
    $pending.Push($full)
    while ($pending.Count -gt 0) {
        foreach ($child in Get-ChildItem -LiteralPath ($pending.Pop()) -Force) {
            if ($child.Attributes -band [System.IO.FileAttributes]::ReparsePoint) {
                throw 'Owned temporary directory contains a reparse point'
            }
            if ($child.PSIsContainer) { $pending.Push($child.FullName) }
        }
    }
    Remove-Item -LiteralPath $full -Recurse -Force
}

try {
    foreach ($name in @('relayd.exe', 'relay.exe')) {
        if (-not (Test-Path -LiteralPath (Join-Path $binaryRoot $name) -PathType Leaf)) {
            throw "Required release binary is missing: $name"
        }
    }
    $tempItem = Get-Item -LiteralPath $tempRoot -Force
    if ($tempItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) {
        throw 'The temporary root is a reparse point'
    }
    $report.host_class = Get-HostClass
    $report.binaries = [ordered]@{
        relayd_sha256 = (Get-FileHash -LiteralPath $daemonPath -Algorithm SHA256).Hash.ToLowerInvariant()
        relay_sha256 = (Get-FileHash -LiteralPath $cliPath -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    Set-Stage 'fixture_creation'
    for ($run = 1; $run -le $Runs; $run++) {
    $ownedRoot = Join-Path $tempRoot ("relay-portable-benchmark-{0}" -f ([Guid]::NewGuid().ToString('N')))
    $ownedFull = [System.IO.Path]::GetFullPath($ownedRoot)
    if ([System.IO.Path]::GetDirectoryName($ownedFull) -ne $tempRoot -or
        [System.IO.Path]::GetFileName($ownedFull) -notmatch '^relay-portable-benchmark-[0-9a-f]{32}$' -or
        (Test-Path -LiteralPath $ownedFull)) {
        throw 'Could not create a unique owned temporary directory'
    }
    if ($outputFull -eq $ownedFull -or $outputFull.StartsWith($ownedFull + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'OutputPath must be outside the owned temporary directory'
    }
    $null = New-Item -ItemType Directory -Path $ownedFull
    $stateDir = Join-Path $ownedFull 'state'
    $rootA = Join-Path $ownedFull 'project-alpha'
    $rootB = Join-Path $ownedFull 'project-bravo'
    $null = New-Item -ItemType Directory -Path $stateDir, $rootA, $rootB
    $priorState = [Environment]::GetEnvironmentVariable('RELAY_STATE_DIR', 'Process')
    $priorInstance = [Environment]::GetEnvironmentVariable('RELAY_INSTANCE', 'Process')
    $env:RELAY_STATE_DIR = $stateDir
    $env:RELAY_INSTANCE = 'portable-benchmark-' + ([Guid]::NewGuid().ToString('N')).Substring(0, 12)
    $daemon = $null
    try {
    Set-Stage "run_${run}_fixture_creation"
    Write-Host "Run $run of ${Runs}: creating two temporary $FilesPerProject-file projects"
    New-SyntheticProject $rootA
    New-SyntheticProject $rootB
    $daemonStderr = Join-Path $ownedFull 'daemon-stderr.json'
    $daemonStdout = Join-Path $ownedFull 'daemon-stdout.json'
    try { $daemon = Start-Process -FilePath $daemonPath -PassThru -WindowStyle Hidden -RedirectStandardError $daemonStderr -RedirectStandardOutput $daemonStdout }
    catch {
        $script:benchmarkFailureCode = 'BENCHMARK_DAEMON_LAUNCH_FAILED'
        throw
    }
    $hostState = Wait-Daemon $daemon $daemonStderr

    Set-Stage "run_${run}_baseline"
    $baseline = @()
    foreach ($pair in @(@('PRJ-bench-alpha', $rootA), @('PRJ-bench-bravo', $rootB))) {
        $null = Invoke-Relay 'project.import' @{ id = $pair[0]; name = $pair[0]; root_path = $pair[1] } $true
        $built = Invoke-Relay 'project.index.build' @{ project_id = $pair[0] } $true
        if ($built.file_count -ne $FilesPerProject) {
            $script:benchmarkFailureCode = 'BENCHMARK_BASELINE_FILE_COUNT'
            throw 'Baseline file count was unexpected'
        }
        $baseline += [int]$built.elapsed_ms
    }
    Start-Sleep -Milliseconds 500
    $attachment = @()
    $metadata = @()
    foreach ($id in @('PRJ-bench-alpha', 'PRJ-bench-bravo')) {
        $verified = Invoke-Relay 'project.index.reconcile' @{ project_id = $id; verify_content = $true } $true
        if ($verified.files_hashed -ne $FilesPerProject) { throw 'Full verification work count was unexpected' }
        $attachment += [int]$verified.elapsed_ms
        $clean = Invoke-Relay 'project.index.reconcile' @{ project_id = $id } $true
        if ($clean.files_hashed -ne 0) { throw 'Clean metadata pass hashed content unexpectedly' }
        $metadata += [int]$clean.elapsed_ms
    }

    Set-Stage "run_${run}_idle_sample"
    $before = (Get-Process -Id $daemon.Id).TotalProcessorTime.TotalMilliseconds
    Start-Sleep -Seconds 5
    $process = Get-Process -Id $daemon.Id
    $idleCpuMs = [Math]::Max(0, $process.TotalProcessorTime.TotalMilliseconds - $before)
    $idleRssBytes = [uint64]$process.WorkingSet64

    Set-Stage "run_${run}_watcher_change"
    $changed = Join-Path $rootA 'set-00\file-0000.txt'
    $clock = [System.Diagnostics.Stopwatch]::StartNew()
    [System.IO.File]::WriteAllText($changed, 'one changed generic project file')
    $staleMs = $null
    while ($clock.Elapsed.TotalSeconds -lt 15) {
        $capabilities = Invoke-Relay 'project.capabilities' @{ project_id = 'PRJ-bench-alpha' }
        if ($capabilities.index_status -eq 'stale') {
            $staleMs = [int]$clock.ElapsedMilliseconds
            break
        }
        Start-Sleep -Milliseconds 25
    }
    if ($null -eq $staleMs) { throw 'Watcher did not mark the changed project stale' }
    $bravo = Invoke-Relay 'project.capabilities' @{ project_id = 'PRJ-bench-bravo' }
    if ($bravo.index_status -ne 'ready') { throw 'Unchanged project lost ready state' }
    $afterHint = Invoke-Relay 'project.index.reconcile' @{ project_id = 'PRJ-bench-alpha' } $true
    if ($afterHint.files_hashed -ne 0) { throw 'Post-hint reconcile hashed content unexpectedly' }
    $full = Invoke-Relay 'project.index.reconcile' @{ project_id = 'PRJ-bench-alpha'; verify_content = $true } $true
    if ($full.files_hashed -ne $FilesPerProject) { throw 'Explicit full verification work count was unexpected' }

    $databaseBytes = [uint64]0
    foreach ($name in @('relay.sqlite3', 'relay.sqlite3-wal', 'relay.sqlite3-shm')) {
        $file = Join-Path $stateDir $name
        if (Test-Path -LiteralPath $file) { $databaseBytes += (Get-Item -LiteralPath $file).Length }
    }
    $sample = [ordered]@{
        run = $run
        daemon_version = $hostState.version
        storage_schema_version = $hostState.storage_schema_version
        correctness = [ordered]@{
            both_baselines_complete = $true
            attachment_verified_content = $true
            clean_metadata_hashed_zero = $true
            watcher_observed_change = $true
            other_project_remained_ready = $true
            post_hint_hashed_zero = $true
            explicit_full_verify_complete = $true
        }
        measures = [ordered]@{
            baseline_ms = $baseline
            attachment_verify_ms = $attachment
            metadata_reconcile_ms = $metadata
            one_file_to_stale_wall_ms = $staleMs
            post_hint_reconcile_ms = [int]$afterHint.elapsed_ms
            full_content_verify_ms = [int]$full.elapsed_ms
            idle_sample_ms = 5000
            idle_process_cpu_ms = $idleCpuMs
            idle_working_set_bytes = $idleRssBytes
            sqlite_main_wal_shm_bytes = $databaseBytes
        }
    }
    $report.samples += $sample
    $report.runs_completed = $run
    Set-Stage "run_${run}_complete"
    }
    finally {
    if ($daemon -and -not $daemon.HasExited) {
        try { $null = Invoke-Relay -Command 'system.shutdown' -Arguments @{} -TimeoutMs 3000 } catch { }
        if (-not $daemon.WaitForExit(3000)) {
            Stop-Process -Id $daemon.Id -Force -ErrorAction SilentlyContinue
            $daemon.WaitForExit()
        }
    }
    if ($null -eq $priorState) { Remove-Item Env:RELAY_STATE_DIR -ErrorAction SilentlyContinue }
    else { $env:RELAY_STATE_DIR = $priorState }
    if ($null -eq $priorInstance) { Remove-Item Env:RELAY_INSTANCE -ErrorAction SilentlyContinue }
    else { $env:RELAY_INSTANCE = $priorInstance }
    Remove-OwnedRoot $ownedFull
    }
    }
    $report.status = 'passed'
    $report.current_stage = 'complete'
    Save-Report
    Write-Host "Local JSON report written to $outputFull"
}
catch {
    $report.status = 'failed'
    $report.failure_code = if ($script:cliTimedOut) { 'BENCHMARK_CLI_TIMEOUT' }
        elseif ($script:benchmarkFailureCode) { $script:benchmarkFailureCode }
        elseif ($report.current_stage -eq 'preflight') { 'BENCHMARK_PREFLIGHT_FAILED' }
        else { 'BENCHMARK_STAGE_FAILED' }
    try { Save-Report } catch { }
    throw "Benchmark failed at $($report.current_stage) ($($report.failure_code)); inspect the local JSON report"
}
finally {
    if ($report.status -eq 'in_progress') {
        $report.status = 'interrupted'
        $report.failure_code = 'BENCHMARK_INTERRUPTED'
        try { Save-Report } catch { }
    }
}
