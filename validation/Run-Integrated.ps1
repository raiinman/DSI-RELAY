param(
    [string]$BinaryDirectory = (Join-Path $PSScriptRoot '..\target\release'),
    [string]$OutputPath = (Join-Path (Get-Location) ("relay-integrated-{0}-{1}.json" -f (Get-Date -Format 'yyyyMMdd-HHmmss'), ([Guid]::NewGuid().ToString('N').Substring(0, 8)))),
    [string]$ProjectRoot,
    [string]$ProjectId,
    [string]$AssetManifestPath,
    [string]$ChangedPath,
    [string]$BlendPath,
    [string]$VerseCapturePath,
    [string]$VerseProjectRelativePath,
    [string]$VerseSessionId,
    [ValidateSet('unknown', 'available', 'unavailable')][string]$UefnAvailability = 'unknown',
    [ValidateSet('unknown', 'available', 'unavailable')][string]$BlenderAvailability = 'unknown',
    [ValidateSet('unknown', 'available', 'unavailable')][string]$KritaAvailability = 'unknown',
    [ValidateSet('unknown', 'available', 'unavailable')][string]$MinimumPcAvailability = 'unknown',
    [ValidateSet('unknown', 'minimum_candidate', 'recommended_candidate')][string]$SupportedHostTier = 'unknown',
    [ValidateRange(1000, 600000)][int]$CommandTimeoutMs = 180000,
    [ValidateRange(1000, 60000)][int]$HostReadyTimeoutMs = 15000
)

$ErrorActionPreference = 'Stop'
$utf8 = New-Object System.Text.UTF8Encoding($false)
$runId = 'RUN-' + [Guid]::NewGuid().ToString('N')
$startedUnixMs = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$planPath = Join-Path $PSScriptRoot 'run-plan.json'
$plan = Get-Content -LiteralPath $planPath -Raw -Encoding UTF8 | ConvertFrom-Json
$known = @(
    'host.health', 'project.import_index', 'project.supported_host_resource', 'project.lifecycle', 'context.result_storage',
    'project.foreground_interference',
    'context.compile_multi_result', 'context.task_snapshot', 'context.conflict_handling', 'context.cost_benchmark',
    'dashboard.browser', 'dashboard.approval_review', 'dashboard.accessibility',
    'uefn.static_inspection', 'uefn.mcp_discovery', 'uefn.spawn_audit', 'verse.imported_analysis', 'verse.project_file_analysis',
    'uefn.live_runtime', 'uefn.live_capture_assertions', 'assets.local_links', 'assets.impact_analysis',
    'krita.declared_formats', 'krita.recovery_candidate', 'blender.mesh_validation', 'blender.native_mesh', 'krita.native_export',
    'gateway.local_discovery_overhead', 'gateway.local_result_context', 'gateway.remote_client', 'skills.local_client', 'remote.chatgpt_client',
    'onboarding.clean_account', 'onboarding.recovery', 'automation.pause_resume', 'automation.check_plan', 'automation.declared_index_checks', 'automation.direct_index_fixture', 'hardware.minimum_pc',
    'report.evidence_capture', 'privacy.local_only', 'installer.upgrade_rollback', 'release.public_install'
)
if ($plan.schema_version -ne 1 -or $plan.workflows.Count -ne $known.Count) { throw 'INVALID_RUN_PLAN' }
$seen = @{}
$previousPhase = 0
foreach ($workflow in $plan.workflows) {
    if ($workflow.workflow_id -notin $known -or $seen.ContainsKey($workflow.workflow_id) -or
        $workflow.phase -lt 2 -or $workflow.phase -gt 11 -or
        $workflow.phase -lt $previousPhase -or
        $workflow.execution -notin @('shared_cli', 'manual', 'harness')) { throw 'INVALID_RUN_PLAN' }
    $seen[$workflow.workflow_id] = $true
    $previousPhase = $workflow.phase
}
if ($seen.Count -ne $known.Count) { throw 'INVALID_RUN_PLAN' }

$binaryRoot = [System.IO.Path]::GetFullPath($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($BinaryDirectory))
$cliPath = Join-Path $binaryRoot 'relay.exe'
$daemonPath = Join-Path $binaryRoot 'relayd.exe'
$gatewayPath = Join-Path $binaryRoot 'relay-gateway.exe'
if (-not (Test-Path -LiteralPath $cliPath -PathType Leaf) -or -not (Test-Path -LiteralPath $daemonPath -PathType Leaf)) {
    throw 'RELAY_BINARIES_MISSING'
}
$outputFull = [System.IO.Path]::GetFullPath($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputPath))
$journalFull = $outputFull + '.journal.json'
if (-not (Test-Path -LiteralPath ([System.IO.Path]::GetDirectoryName($outputFull)) -PathType Container) -or
    (Test-Path -LiteralPath $outputFull) -or (Test-Path -LiteralPath $journalFull)) { throw 'OUTPUT_PATH_UNAVAILABLE' }
if ($ProjectRoot) {
    $ProjectRoot = [System.IO.Path]::GetFullPath($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($ProjectRoot))
    if (-not (Test-Path -LiteralPath $ProjectRoot -PathType Container)) { throw 'PROJECT_ROOT_UNAVAILABLE' }
}
if (-not $ProjectId) { $ProjectId = 'VAL-' + [Guid]::NewGuid().ToString('N').Substring(0, 12) }
if ($ProjectId -cnotmatch '^[A-Za-z0-9_.-]{1,100}$') { throw 'PROJECT_ID_INVALID' }
if ($AssetManifestPath -and $ProjectId.StartsWith('VAL-') -and -not $PSBoundParameters.ContainsKey('ProjectId')) {
    throw 'PROJECT_ID_REQUIRED_FOR_MANIFEST'
}
foreach ($path in @($AssetManifestPath, $VerseCapturePath)) {
    if ($path -and -not (Test-Path -LiteralPath $path -PathType Leaf)) { throw 'INPUT_FILE_UNAVAILABLE' }
}
if ($ChangedPath -and ($ChangedPath.Length -gt 512 -or $ChangedPath -match '(^[A-Za-z]:|^[/\\]|(^|[/\\])\.\.([/\\]|$))')) {
    throw 'CHANGED_PATH_INVALID'
}
if ($BlendPath -and ($BlendPath.Length -gt 512 -or $BlendPath -notmatch '(?i)\.blend$' -or
    $BlendPath -match '(^[A-Za-z]:|^[/\\]|\\|(^|/)\.\.(/|$))')) { throw 'BLEND_PATH_INVALID' }
if ($VerseProjectRelativePath -and ($VerseProjectRelativePath.Length -gt 512 -or
    $VerseProjectRelativePath -notmatch '(?i)\.(log|jsonl)$' -or
    $VerseProjectRelativePath -match '(^[A-Za-z]:|^[/\\]|\\|(^|/)\.\.(/|$))')) { throw 'VERSE_PROJECT_PATH_INVALID' }
if ($VerseSessionId -and $VerseSessionId -cnotmatch '^[A-Za-z0-9_.-]{1,64}$') { throw 'VERSE_SESSION_ID_INVALID' }

function Get-SafeCode {
    param([string]$Value, [string]$Fallback = 'COMMAND_FAILED')
    if ($Value -cmatch '^[A-Z][A-Z0-9_]{0,95}$') { return $Value }
    return $Fallback
}

function Get-TextHash {
    param([string]$Value)
    $bytes = $utf8.GetBytes($Value)
    $hash = [System.Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($hash.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant() }
    finally { $hash.Dispose() }
}

function Get-Availability {
    param($Requirement)
    switch ($Requirement.kind) {
        'uefn' { return $UefnAvailability }
        'creator_app' {
            if ($Requirement.app_id -eq 'blender') { return $BlenderAvailability }
            if ($Requirement.app_id -eq 'krita') { return $KritaAvailability }
            return 'unknown'
        }
        'hardware_class' {
            if ($Requirement.class_id -eq 'minimum_pc') { return $MinimumPcAvailability }
            return 'unknown'
        }
        default { return 'unknown' }
    }
}

function Get-ProcessSample {
    param($Process)
    if (-not $Process) { return @{ peak_rss_bytes = $null; working_set_bytes = $null; cpu_ms = $null } }
    try {
        $Process.Refresh()
        return @{
            peak_rss_bytes = [long]$Process.PeakWorkingSet64
            working_set_bytes = [long]$Process.WorkingSet64
            cpu_ms = [long]$Process.TotalProcessorTime.TotalMilliseconds
        }
    } catch { return @{ peak_rss_bytes = $null; working_set_bytes = $null; cpu_ms = $null } }
}

function Get-HostHardwareProbe {
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        $computer = Get-CimInstance -ClassName Win32_ComputerSystem -ErrorAction Stop
        $processors = @(Get-CimInstance -ClassName Win32_Processor -ErrorAction Stop)
        $physical = [long](($processors | Measure-Object -Property NumberOfCores -Sum).Sum)
        $logical = [long](($processors | Measure-Object -Property NumberOfLogicalProcessors -Sum).Sum)
        $memory = [long]$computer.TotalPhysicalMemory
        if ($physical -lt 1 -or $logical -lt $physical -or $memory -lt 1073741824) { throw 'HOST_PROBE_INVALID' }
        return [pscustomobject]@{ available = $true; physical_cores = $physical;
            logical_processors = $logical; memory_bytes = $memory; overhead_ms = [long]$watch.ElapsedMilliseconds }
    } catch {
        return [pscustomobject]@{ available = $false; overhead_ms = [long]$watch.ElapsedMilliseconds }
    } finally { $watch.Stop() }
}

function Initialize-ForegroundProbe {
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        if (-not ('RelayValidationForegroundProbe' -as [type])) {
            Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class RelayValidationForegroundProbe {
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);
}
'@ -ErrorAction Stop
        }
        return [pscustomobject]@{ available = $true; overhead_ms = [long]$watch.ElapsedMilliseconds }
    } catch {
        return [pscustomobject]@{ available = $false; overhead_ms = [long]$watch.ElapsedMilliseconds }
    } finally { $watch.Stop() }
}

function Get-ForegroundCreatorPresence {
    try {
        $window = [RelayValidationForegroundProbe]::GetForegroundWindow()
        if ($window -eq [IntPtr]::Zero) { return $false }
        [uint32]$processId = 0
        $null = [RelayValidationForegroundProbe]::GetWindowThreadProcessId($window, [ref]$processId)
        if ($processId -eq 0) { return $false }
        $name = [System.Diagnostics.Process]::GetProcessById([int]$processId).ProcessName.ToLowerInvariant()
        return ($name -like 'unrealeditorfortnite*' -or $name -eq 'blender' -or $name -eq 'krita')
    } catch { return $null }
}

function Read-BoundedInput {
    param([string]$Path, [long]$MaximumBytes)
    $file = Get-Item -LiteralPath $Path
    if ($file.Length -gt $MaximumBytes) { throw 'INPUT_TOO_LARGE' }
    return [System.IO.File]::ReadAllText($file.FullName, $utf8)
}

function Add-IndexResourceSample {
    param($Capture, $DaemonProcess, $CliProcess, [bool]$ProbeForeground)
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    $daemonSample = Get-ProcessSample $DaemonProcess
    $cliSample = Get-ProcessSample $CliProcess
    $Capture.daemon_peak_working_set_bytes = [Math]::Max([long]$Capture.daemon_peak_working_set_bytes,
        [long]$daemonSample.working_set_bytes)
    $Capture.cli_peak_working_set_bytes = [Math]::Max([long]$Capture.cli_peak_working_set_bytes,
        [long]$cliSample.working_set_bytes)
    if ($null -ne $daemonSample.cpu_ms) { $Capture.last_daemon_cpu_ms = [long]$daemonSample.cpu_ms }
    if ($null -ne $cliSample.cpu_ms) { $Capture.last_cli_cpu_ms = [long]$cliSample.cpu_ms }
    if ($ProbeForeground) {
        $presence = Get-ForegroundCreatorPresence
        if ($null -ne $presence) {
            $Capture.foreground_observed_samples++
            if ($presence) { $Capture.foreground_creator_samples++ }
        }
    }
    $Capture.sample_count++
    $watch.Stop()
    $Capture.sampling_overhead_ms += [long]$watch.ElapsedMilliseconds
}

function Invoke-Relay {
    param([string]$CommandId, [hashtable]$CommandArguments, [bool]$Keyed = $false,
        [int]$CommandVersion = 1, [bool]$SampleResources = $false)
    $requestId = 'VALID-' + [Guid]::NewGuid().ToString('N')
    $request = [ordered]@{
        request_id = $requestId
        command = $CommandId
        command_version = $CommandVersion
        arguments = $CommandArguments
    }
    if ($Keyed) { $request.idempotency_key = 'IDEMP-' + $requestId }
    $child = $null
    $started = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    $beforeDaemon = Get-ProcessSample $script:daemon
    $response = $null
    $errorCode = 'CLI_RESPONSE_INVALID'
    $cliSample = @{ peak_rss_bytes = $null; cpu_ms = $null }
    $requestBytes = $null
    $responseBytes = $null
    $resourceCapture = if ($SampleResources) { [ordered]@{
        daemon_peak_working_set_bytes = 0L; cli_peak_working_set_bytes = 0L
        sample_count = 0; sampling_overhead_ms = 0L
        foreground_observed_samples = 0; foreground_creator_samples = 0
        last_daemon_cpu_ms = $null; last_cli_cpu_ms = $null
    } } else { $null }
    try {
        $inputJson = $request | ConvertTo-Json -Depth 20 -Compress
        $requestBytes = $utf8.GetByteCount($inputJson)
        if ($requestBytes -gt 262144) { throw 'INPUT_TOO_LARGE' }
        $startInfo = New-Object System.Diagnostics.ProcessStartInfo
        $startInfo.FileName = $cliPath
        $startInfo.Arguments = 'exec --stdin'
        $startInfo.UseShellExecute = $false
        $startInfo.CreateNoWindow = $true
        $startInfo.RedirectStandardInput = $true
        $startInfo.RedirectStandardOutput = $true
        $startInfo.RedirectStandardError = $true
        $startInfo.StandardOutputEncoding = $utf8
        $startInfo.StandardErrorEncoding = $utf8
        $child = New-Object System.Diagnostics.Process
        $child.StartInfo = $startInfo
        $null = $child.Start()
        $stdoutTask = $child.StandardOutput.ReadToEndAsync()
        $stderrTask = $child.StandardError.ReadToEndAsync()
        if ($SampleResources) {
            Add-IndexResourceSample $resourceCapture $script:daemon $child $script:foregroundProbeAvailable
        }
        $child.StandardInput.WriteLine($inputJson)
        $child.StandardInput.Close()
        $finished = $false
        if ($SampleResources) {
            $samplingWindow = [System.Diagnostics.Stopwatch]::StartNew()
            Add-IndexResourceSample $resourceCapture $script:daemon $child $script:foregroundProbeAvailable
            while (-not ($finished = $child.WaitForExit(100))) {
                Add-IndexResourceSample $resourceCapture $script:daemon $child $script:foregroundProbeAvailable
                if ($samplingWindow.ElapsedMilliseconds -ge $CommandTimeoutMs) { break }
            }
            Add-IndexResourceSample $resourceCapture $script:daemon $child $script:foregroundProbeAvailable
            if (-not $finished -and $child.HasExited) { $finished = $true }
            $samplingWindow.Stop()
        } else { $finished = $child.WaitForExit($CommandTimeoutMs) }
        if (-not $finished) {
            $errorCode = 'CLI_TIMEOUT'
            try { $child.Kill() } catch { }
            $null = $child.WaitForExit(5000)
        } else {
            $text = $stdoutTask.GetAwaiter().GetResult()
            $responseBytes = $utf8.GetByteCount($text.TrimEnd("`r", "`n"))
            if ($utf8.GetByteCount($text) -gt 1048576) { $errorCode = 'CLI_OUTPUT_TOO_LARGE' }
            else { try { $response = $text | ConvertFrom-Json } catch { $response = $null } }
            if ($response -and $response.request_id -eq $requestId -and $null -ne $response.ok) {
                $script:hadValidatedResponse = $true
            } else { $response = $null }
            if ($response -and $response.ok -eq $true) { $errorCode = $null }
            elseif ($response -and $response.error) { $errorCode = Get-SafeCode ([string]$response.error.code) }
            elseif ($errorCode -ne 'CLI_OUTPUT_TOO_LARGE') { $errorCode = 'CLI_RESPONSE_INVALID' }
        }
        $null = $stderrTask.GetAwaiter().GetResult()
        $cliSample = Get-ProcessSample $child
    } catch {
        if ($errorCode -ne 'CLI_TIMEOUT') { $errorCode = 'CLI_EXECUTION_FAILED' }
    } finally {
        if ($child) {
            if (-not $child.HasExited) { try { $child.Kill() } catch { } }
            try { $null = $child.WaitForExit(5000) } catch { }
            $child.Dispose()
        }
    }
    $afterDaemon = Get-ProcessSample $script:daemon
    $duration = [Math]::Max(0, ([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() - $started))
    $lastDaemonCpu = if ($null -ne $afterDaemon.cpu_ms) { $afterDaemon.cpu_ms }
        elseif ($SampleResources) { $resourceCapture.last_daemon_cpu_ms } else { $null }
    $daemonCpu = if ($null -ne $beforeDaemon.cpu_ms -and $null -ne $lastDaemonCpu) {
        [Math]::Max(0, ($lastDaemonCpu - $beforeDaemon.cpu_ms))
    } else { 0 }
    $cliCpu = if ($null -ne $cliSample.cpu_ms) { $cliSample.cpu_ms }
        elseif ($SampleResources -and $null -ne $resourceCapture.last_cli_cpu_ms) {
            $resourceCapture.last_cli_cpu_ms
        } else { 0 }
    $peak = [Math]::Max([long]$cliSample.peak_rss_bytes, [long]$afterDaemon.peak_rss_bytes)
    if ($SampleResources) {
        $resourceCapture.daemon_peak_working_set_bytes = [Math]::Max(
            [long]$resourceCapture.daemon_peak_working_set_bytes, [long]$afterDaemon.working_set_bytes)
        $resourceCapture.cli_peak_working_set_bytes = [Math]::Max(
            [long]$resourceCapture.cli_peak_working_set_bytes, [long]$cliSample.working_set_bytes)
        $resourceCapture.daemon_cpu_ms = [long]$daemonCpu
        $resourceCapture.cli_cpu_ms = [long]$cliCpu
        $resourceCapture.index_command_elapsed_ms = [long]$duration
    }
    $entry = [ordered]@{
        command_id = $CommandId
        ok = ($null -eq $errorCode)
        error_code = $errorCode
        duration_ms = [long]$duration
        request_bytes = $requestBytes
        response_bytes = $responseBytes
        peak_rss_bytes = if ($peak -gt 0) { [long]$peak } else { $null }
        cpu_ms = [long]($daemonCpu + $cliCpu)
    }
    $null = $script:activeCommands.Add($entry)
    return [pscustomobject]@{ ok = ($null -eq $errorCode); result = $response.result; error_code = $errorCode;
        transport_metric = [ordered]@{ path_id = 'cli_stdin'; byte_scope = 'application_json';
            request_bytes = $requestBytes; response_bytes = $responseBytes; elapsed_ms = [long]$duration };
        resource_capture = $resourceCapture }
}

function Invoke-LocalMcpRead {
    param(
        [ValidateSet('relay_registry_list', 'relay_result_list', 'relay_result_describe', 'relay_result_context')][string]$ToolName = 'relay_registry_list',
        [hashtable]$Arguments
    )
    if (-not $Arguments) {
        $Arguments = @{ prefix = 'registry.'; limit = 8 }
    }
    $gateway = $null
    $client = $null
    $credential = $null
    $attempted = $false
    $requestBytes = 0L
    $responseBytes = 0L
    $elapsed = 0L
    $watch = $null
    $reason = 'LOCAL_GATEWAY_UNAVAILABLE'
    $result = $null
    try {
        if (-not (Test-Path -LiteralPath $gatewayPath -PathType Leaf)) {
            return [pscustomobject]@{ ok = $false; attempted = $false; reason = 'GATEWAY_BINARY_MISSING' }
        }
        $script:componentVersions += [ordered]@{
            component_id = 'relay_gateway_sha256'
            version = (Get-FileHash -LiteralPath $gatewayPath -Algorithm SHA256).Hash.ToLowerInvariant()
        }
        if (-not $env:LOCALAPPDATA) { throw 'GATEWAY_CREDENTIAL_UNAVAILABLE' }
        $credentialPath = Join-Path $env:LOCALAPPDATA 'DSI\RELAY\gateway.auth.dpapi'
        $gatewayStarted = [DateTime]::UtcNow
        $gateway = Start-Process -FilePath $gatewayPath -PassThru -WindowStyle Hidden `
            -RedirectStandardOutput (Join-Path $script:ownedFull 'gateway.stdout') `
            -RedirectStandardError (Join-Path $script:ownedFull 'gateway.stderr')
        $deadline = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() + $HostReadyTimeoutMs
        $ready = $false
        while ([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() -lt $deadline -and -not $gateway.HasExited) {
            $item = Get-Item -LiteralPath $credentialPath -ErrorAction SilentlyContinue
            if ($item -and $item.Length -le 4096 -and $item.LastWriteTimeUtc -ge $gatewayStarted) {
                $ready = $true
                break
            }
            Start-Sleep -Milliseconds 50
        }
        if (-not $ready -or $gateway.HasExited) {
            return [pscustomobject]@{ ok = $false; attempted = $false; reason = 'LOCAL_GATEWAY_UNAVAILABLE' }
        }
        $protected = [System.IO.File]::ReadAllBytes($credentialPath)
        if ($protected.Length -gt 4096) { throw 'GATEWAY_CREDENTIAL_INVALID' }
        $plaintext = [System.Security.Cryptography.ProtectedData]::Unprotect(
            $protected, $null, [System.Security.Cryptography.DataProtectionScope]::CurrentUser)
        try { $credential = $utf8.GetString($plaintext) | ConvertFrom-Json }
        finally { [Array]::Clear($plaintext, 0, $plaintext.Length) }
        if ($credential.format_version -ne 1 -or $credential.port -ne 8765 -or
            $credential.token -cnotmatch '^[0-9a-fA-F]{64}$') { throw 'GATEWAY_CREDENTIAL_INVALID' }

        $body = [ordered]@{
            jsonrpc = '2.0'; id = 1; method = 'tools/call'; params = [ordered]@{
                name = $ToolName; arguments = $Arguments
                _meta = [ordered]@{
                    'io.modelcontextprotocol/protocolVersion' = '2026-07-28'
                    'io.modelcontextprotocol/clientInfo' = [ordered]@{ name = 'relay-validation'; version = '1' }
                    'io.modelcontextprotocol/clientCapabilities' = @{}
                }
            }
        } | ConvertTo-Json -Depth 12 -Compress
        $requestBytes = [long]$utf8.GetByteCount($body)
        if ($requestBytes -gt 8192) { throw 'MCP_REQUEST_TOO_LARGE' }
        $request = "POST /mcp HTTP/1.1`r`nHost: 127.0.0.1:8765`r`nAuthorization: Bearer $($credential.token)`r`nContent-Type: application/json`r`nAccept: application/json, text/event-stream`r`nMCP-Protocol-Version: 2026-07-28`r`nMcp-Method: tools/call`r`nMcp-Name: $ToolName`r`nContent-Length: $requestBytes`r`nConnection: close`r`n`r`n$body"
        $requestWire = $utf8.GetBytes($request)
        $client = New-Object System.Net.Sockets.TcpClient
        $client.SendTimeout = 3000
        $client.ReceiveTimeout = 3000
        $watch = [System.Diagnostics.Stopwatch]::StartNew()
        $client.Connect('127.0.0.1', 8765)
        $stream = $client.GetStream()
        $attempted = $true
        $stream.Write($requestWire, 0, $requestWire.Length)
        $buffer = New-Object byte[] 4096
        $received = New-Object System.IO.MemoryStream
        try {
            while (($count = $stream.Read($buffer, 0, $buffer.Length)) -gt 0) {
                if ($received.Length + $count -gt 69632) { throw 'MCP_RESPONSE_TOO_LARGE' }
                $received.Write($buffer, 0, $count)
            }
            $wire = $received.ToArray()
        } finally { $received.Dispose() }
        $watch.Stop()
        $elapsed = [long]$watch.ElapsedMilliseconds
        $responseText = $utf8.GetString($wire)
        $separator = $responseText.IndexOf("`r`n`r`n", [StringComparison]::Ordinal)
        if ($separator -lt 0 -or -not $responseText.StartsWith('HTTP/1.1 200 ', [StringComparison]::Ordinal)) {
            throw 'MCP_RESPONSE_INVALID'
        }
        $responseBody = $responseText.Substring($separator + 4)
        $responseBytes = [long]$utf8.GetByteCount($responseBody)
        $result = $responseBody | ConvertFrom-Json
        $content = $result.result.structuredContent
        $shapeValid = switch ($ToolName) {
            'relay_registry_list' { $null -ne $content.commands }
            'relay_result_list' {
                $null -ne $content.results -and @($content.results | Where-Object {
                    $_.project_id -ne $Arguments.project_id
                }).Count -eq 0
            }
            'relay_result_describe' {
                $content.id -eq $Arguments.result_id -and $content.project_id -eq $Arguments.project_id
            }
            'relay_result_context' {
                $content.result_id -eq $Arguments.result_id -and $null -ne $content.facts
            }
        }
        if ($result.jsonrpc -ne '2.0' -or $result.id -ne 1 -or $result.error -or
            $result.result.isError -eq $true -or -not $shapeValid) {
            throw 'MCP_RESPONSE_INVALID'
        }
        $reason = if ($ToolName -eq 'relay_registry_list') { 'LOCAL_DISCOVERY_MEASURED' } else { 'LOCAL_RESULT_READ' }
        return [pscustomobject]@{ ok = $true; attempted = $true; reason = $reason; result = $content;
            transport_metric = [ordered]@{ path_id = 'local_mcp_http'; byte_scope = 'application_json';
                request_bytes = $requestBytes; response_bytes = $responseBytes; elapsed_ms = $elapsed } }
    } catch {
        $reason = if ($attempted) { 'LOCAL_MCP_REQUEST_FAILED' } else { 'LOCAL_GATEWAY_UNAVAILABLE' }
        if ($watch) { $elapsed = [long]$watch.ElapsedMilliseconds }
        $metric = if ($attempted) { [ordered]@{ path_id = 'local_mcp_http'; byte_scope = 'application_json';
            request_bytes = $requestBytes; response_bytes = $responseBytes; elapsed_ms = $elapsed } } else { $null }
        return [pscustomobject]@{ ok = $false; attempted = $attempted; reason = $reason; transport_metric = $metric }
    } finally {
        if ($client) { $client.Dispose() }
        if ($gateway) {
            if (-not $gateway.HasExited) { try { $gateway.Kill() } catch { } }
            try { $null = $gateway.WaitForExit(5000) } catch { }
            $gateway.Dispose()
        }
        if ($attempted) {
            $null = $script:activeCommands.Add([ordered]@{
                command_id = switch ($ToolName) {
                    'relay_registry_list' { 'registry.list' }
                    'relay_result_list' { 'result.list' }
                    'relay_result_describe' { 'result.describe' }
                    'relay_result_context' { 'result.context' }
                };
                ok = ($reason -in @('LOCAL_DISCOVERY_MEASURED', 'LOCAL_RESULT_READ'));
                error_code = if ($reason -in @('LOCAL_DISCOVERY_MEASURED', 'LOCAL_RESULT_READ')) { $null } else { $reason };
                duration_ms = $elapsed; request_bytes = $requestBytes; response_bytes = $responseBytes;
                peak_rss_bytes = $null; cpu_ms = 0
            })
        }
    }
}

function Complete-Workflow {
    param($Workflow, [string]$Status, [string]$ReasonCode, [bool]$EvidenceEligible = $false)
    $requirements = @()
    $unavailable = $false
    $unknown = $false
    foreach ($requirement in @($Workflow.requires)) {
        $availability = Get-Availability $requirement
        $requirements += [ordered]@{ requirement = $requirement; availability = $availability }
        if ($availability -eq 'unavailable') { $unavailable = $true }
        if ($availability -eq 'unknown') { $unknown = $true }
    }
    if ($Status -eq 'failed' -and $script:activeCommands.Count -gt 0) { }
    elseif ($unavailable) { $Status = 'untested'; $ReasonCode = 'REQUIRED_ENVIRONMENT_UNAVAILABLE' }
    elseif ($unknown) { $Status = 'untested'; $ReasonCode = 'REQUIRED_ENVIRONMENT_UNKNOWN' }
    elseif (-not $Status) { $Status = 'untested'; $ReasonCode = 'NOT_EXECUTED' }
    elseif ($Status -eq 'passed' -and -not $EvidenceEligible) {
        $Status = 'untested'; $ReasonCode = 'OBSERVED_EVIDENCE_REQUIRED'
    }
    $ReasonCode = Get-SafeCode $ReasonCode 'UNKNOWN_OUTCOME'
    $commands = @($script:activeCommands.ToArray())
    $journalEntry = [ordered]@{
        workflow_id = [string]$Workflow.workflow_id
        status = $Status
        reason_code = $ReasonCode
        commands = $commands
    }
    $entryJson = $journalEntry | ConvertTo-Json -Depth 12 -Compress
    $entryHash = Get-TextHash $entryJson
    $null = $script:journal.Add([ordered]@{ workflow_id = $Workflow.workflow_id; entry_json = $entryJson; entry_sha256 = $entryHash })
    $evidence = if ($EvidenceEligible -and $commands.Count -gt 0) {
        [ordered]@{ kind = 'observed'; source_id = 'relay_runner'; log_ref = ('sha256:' + $entryHash) }
    } else { [ordered]@{ kind = 'none' } }
    $cpu = 0L
    $peak = 0L
    $codes = @()
    $steps = @()
    foreach ($entry in $commands) {
        $cpu += [long]$entry.cpu_ms
        if ($entry.peak_rss_bytes -gt $peak) { $peak = [long]$entry.peak_rss_bytes }
        if ($entry.error_code) { $codes += $entry.error_code }
        $steps += $entry.command_id
    }
    $timing = if ($commands.Count -gt 0) { [ordered]@{
        started_unix_ms = [long]$script:workflowStarted
        duration_ms = [long]([Math]::Max(0, [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() - $script:workflowStarted))
    } } else { $null }
    $resource = if ($commands.Count -gt 0) { [ordered]@{
        peak_rss_bytes = if ($peak -gt 0) { $peak } else { $null }
        cpu_ms = $cpu
        io_read_bytes = $null
        io_write_bytes = $null
        peak_gpu_bytes = $null
    } } else { $null }
    $reproduction = if ($steps.Count -gt 0) { [ordered]@{
        command_id = $steps[0]
        scenario_ref = [string]$Workflow.workflow_id
        step_codes = @($steps | Select-Object -First 16)
    } } else { $null }
    $workflowResult = [ordered]@{
        workflow_id = [string]$Workflow.workflow_id
        phase = [int]$Workflow.phase
        status = $Status
        reason_code = $ReasonCode
        requirements = $requirements
        evidence = $evidence
        diagnostic_codes = @($codes | Select-Object -Unique -First 32)
        component_versions = $script:componentVersions
        timing = $timing
        resource_use = $resource
        reproduction = $reproduction
    }
    if ($script:transportMetrics.Count -gt 0) {
        $workflowResult.transport_metrics = @($script:transportMetrics.ToArray())
    }
    if ($script:contextCostMetric) {
        $workflowResult.context_cost_metric = $script:contextCostMetric
    }
    if ($script:hostResourceMetric) {
        $workflowResult.host_resource_metric = $script:hostResourceMetric
    }
    return $workflowResult
}

function Require-Relay {
    param([string]$CommandId, [hashtable]$CommandArguments, [bool]$Keyed = $false, [int]$CommandVersion = 1)
    $exchange = Invoke-Relay $CommandId $CommandArguments $Keyed $CommandVersion
    if (-not $exchange.ok) {
        $script:lastFailureCode = Get-SafeCode $exchange.error_code
        throw 'WORKFLOW_COMMAND_FAILED'
    }
    return $exchange.result
}

function Write-ReservedJson {
    param([string]$Destination, $Value)
    $temporary = Join-Path ([System.IO.Path]::GetDirectoryName($Destination)) ('.relay-integrated-' + [Guid]::NewGuid().ToString('N') + '.tmp')
    try {
        $bytes = $utf8.GetBytes(($Value | ConvertTo-Json -Depth 24) + "`n")
        $stream = New-Object System.IO.FileStream($temporary, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
        try { $stream.Write($bytes, 0, $bytes.Length) }
        finally { $stream.Dispose() }
        [System.IO.File]::Replace($temporary, $Destination, $null)
    } finally {
        if (Test-Path -LiteralPath $temporary -PathType Leaf) { Remove-Item -LiteralPath $temporary -Force }
    }
}

function Test-OwnedTempTarget {
    param([string]$Candidate, [bool]$RequireMarker)
    try {
        $item = Get-Item -LiteralPath $Candidate -Force -ErrorAction Stop
        $resolved = [System.IO.Path]::GetFullPath($item.FullName)
        if ([System.IO.Path]::GetDirectoryName($resolved) -ine $tempRoot -or
            [System.IO.Path]::GetFileName($resolved) -cnotmatch '^relay-integrated-run-[0-9a-f]{32}$' -or
            -not $item.PSIsContainer -or
            ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint)) { return $false }
        $marker = Join-Path $resolved '.relay-integrated-owner'
        if ($RequireMarker -and (-not (Test-Path -LiteralPath $marker -PathType Leaf) -or
            [System.IO.File]::ReadAllText($marker, $utf8) -cne 'RELAY_INTEGRATED_TEMP_V1')) { return $false }
        $pending = New-Object 'System.Collections.Generic.Queue[string]'
        $pending.Enqueue($resolved)
        while ($pending.Count -gt 0) {
            foreach ($child in (Get-ChildItem -LiteralPath $pending.Dequeue() -Force -ErrorAction Stop)) {
                if ($child.Attributes -band [System.IO.FileAttributes]::ReparsePoint) { return $false }
                if ($child.PSIsContainer) { $pending.Enqueue($child.FullName) }
            }
        }
        return $true
    } catch { return $false }
}

function Clear-OrphanValidationRoots {
    $cutoff = [DateTime]::UtcNow.AddHours(-24)
    foreach ($candidate in (Get-ChildItem -LiteralPath $tempRoot -Directory -Filter 'relay-integrated-run-*' -Force -ErrorAction SilentlyContinue)) {
        if ($candidate.LastWriteTimeUtc -gt $cutoff -or -not (Test-OwnedTempTarget $candidate.FullName $true)) { continue }
        $hostPath = Join-Path $candidate.FullName 'state\host.json'
        if (Test-Path -LiteralPath $hostPath -PathType Leaf) {
            try {
                $hostState = Get-Content -LiteralPath $hostPath -Raw -Encoding UTF8 | ConvertFrom-Json
                $hostPid = 0
                if (-not [int]::TryParse([string]$hostState.pid, [ref]$hostPid)) { continue }
                if (Get-Process -Id $hostPid -ErrorAction SilentlyContinue) { continue }
            } catch { continue }
        }
        try { Remove-Item -LiteralPath $candidate.FullName -Recurse -Force -ErrorAction Stop } catch { }
    }
}

function Reserve-JsonPath {
    param([string]$Destination)
    $placeholder = ([ordered]@{ status = 'incomplete'; run_id = $runId; reason_code = 'RUNNER_INTERRUPTED' } | ConvertTo-Json -Compress) + "`n"
    $bytes = $utf8.GetBytes($placeholder)
    $stream = New-Object System.IO.FileStream($Destination, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
    try { $stream.Write($bytes, 0, $bytes.Length); $stream.Flush() }
    finally { $stream.Dispose() }
}

Reserve-JsonPath $outputFull
try { Reserve-JsonPath $journalFull }
catch { throw 'OUTPUT_PATH_UNAVAILABLE' }

$tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\')
Clear-OrphanValidationRoots
$ownedFull = [System.IO.Path]::GetFullPath((Join-Path $tempRoot ('relay-integrated-run-' + [Guid]::NewGuid().ToString('N'))))
if ([System.IO.Path]::GetDirectoryName($ownedFull) -ne $tempRoot -or
    [System.IO.Path]::GetFileName($ownedFull) -cnotmatch '^relay-integrated-run-[0-9a-f]{32}$' -or
    $outputFull.StartsWith($ownedFull + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'TEMP_ROOT_INVALID' }

$previousState = [Environment]::GetEnvironmentVariable('RELAY_STATE_DIR', 'Process')
$previousInstance = [Environment]::GetEnvironmentVariable('RELAY_INSTANCE', 'Process')
$script:daemon = $null
$script:daemonReady = $false
$script:ownedFull = $ownedFull
$script:journal = New-Object 'System.Collections.Generic.List[object]'
$script:results = New-Object 'System.Collections.Generic.List[object]'
$script:relayVersion = 'unknown'
$script:componentVersions = @(
    [ordered]@{ component_id = 'relay_cli_sha256'; version = (Get-FileHash -LiteralPath $cliPath -Algorithm SHA256).Hash.ToLowerInvariant() },
    [ordered]@{ component_id = 'relayd_sha256'; version = (Get-FileHash -LiteralPath $daemonPath -Algorithm SHA256).Hash.ToLowerInvariant() }
)
$script:projectReady = $false
$script:indexMetrics = $null
$script:firstResultId = $null
$script:secondResultId = $null
$script:manifestText = $null
$lifeId = $ProjectId + '-LIFE'
if ($lifeId.Length -gt 128) { throw 'PROJECT_ID_INVALID' }

try {
    $acl = New-Object System.Security.AccessControl.DirectorySecurity
    $currentSid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User
    $acl.SetAccessRuleProtection($true, $false)
    $rule = New-Object System.Security.AccessControl.FileSystemAccessRule($currentSid, 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow')
    $acl.AddAccessRule($rule)
    $ownedDirectory = [System.IO.Directory]::CreateDirectory($ownedFull)
    [System.IO.FileSystemAclExtensions]::SetAccessControl($ownedDirectory, $acl)
    [System.IO.File]::WriteAllText((Join-Path $ownedFull '.relay-integrated-owner'), 'RELAY_INTEGRATED_TEMP_V1', $utf8)
    $stateDir = Join-Path $ownedFull 'state'
    $null = [System.IO.Directory]::CreateDirectory($stateDir)
    $env:RELAY_STATE_DIR = $stateDir
    $env:RELAY_INSTANCE = 'integrated-' + [Guid]::NewGuid().ToString('N').Substring(0, 12)
    $daemonStdout = Join-Path $ownedFull 'daemon.stdout'
    $daemonStderr = Join-Path $ownedFull 'daemon.stderr'
    try {
        $script:daemon = Start-Process -FilePath $daemonPath -PassThru -WindowStyle Hidden `
            -RedirectStandardOutput $daemonStdout -RedirectStandardError $daemonStderr
        $deadline = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() + $HostReadyTimeoutMs
        while ([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() -lt $deadline -and -not $script:daemon.HasExited) {
            $hostPath = Join-Path $stateDir 'host.json'
            if (Test-Path -LiteralPath $hostPath -PathType Leaf) {
                try {
                    $hostState = Get-Content -LiteralPath $hostPath -Raw -Encoding UTF8 | ConvertFrom-Json
                    if ($hostState.pid -eq $script:daemon.Id) { $script:daemonReady = $true; break }
                } catch { }
            }
            Start-Sleep -Milliseconds 50
        }
    } catch { $script:daemonReady = $false }

    foreach ($workflow in $plan.workflows) {
        $script:activeCommands = New-Object 'System.Collections.Generic.List[object]'
        $script:transportMetrics = New-Object 'System.Collections.Generic.List[object]'
        $script:contextCostMetric = $null
        $script:hostResourceMetric = $null
        $script:workflowStarted = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
        $script:lastFailureCode = 'RUNNER_EXCEPTION'
        $script:hadValidatedResponse = $false
        $status = $null
        $reason = $null
        $evidenceEligible = $false
        try {
            $missingRequirement = @($workflow.requires | Where-Object { (Get-Availability $_) -ne 'available' }).Count -gt 0
            if ($missingRequirement) {
                $status = 'untested'
                $reason = 'REQUIRED_ENVIRONMENT_UNKNOWN'
            } elseif (-not $script:daemonReady -and $workflow.execution -ne 'manual') {
                $status = if ($workflow.workflow_id -eq 'host.health') { 'failed' } else { 'blocked' }
                $reason = if ($workflow.workflow_id -eq 'host.health') { 'DAEMON_START_FAILED' } else { 'HOST_UNAVAILABLE' }
            } else {
                switch ($workflow.workflow_id) {
                    'host.health' {
                        $statusResult = Require-Relay 'system.status' @{}
                        $doctor = Require-Relay 'system.doctor' @{}
                        $diagnostics = Require-Relay 'diagnostics.summary' @{}
                        if ($statusResult.version -cmatch '^[A-Za-z0-9_.+-]{1,64}$') {
                            $script:relayVersion = [string]$statusResult.version
                            $script:componentVersions += [ordered]@{ component_id = 'relay'; version = $script:relayVersion }
                        }
                        $status = if ($statusResult.recovery_state -eq 'Healthy' -and $doctor.healthy -eq $true -and $diagnostics.available -eq $true) { 'passed' } else { 'failed' }
                        $reason = if ($status -eq 'passed') { 'HEALTH_CHECKS_PASSED' } else { 'HEALTH_CHECKS_FAILED' }
                        $evidenceEligible = $true
                    }
                    'project.import_index' {
                        if (-not $ProjectRoot) { $status = 'untested'; $reason = 'PROJECT_ROOT_NOT_SUPPLIED'; break }
                        $null = Require-Relay 'project.import' @{ id = $ProjectId; name = 'Integrated Validation Project'; root_path = $ProjectRoot } $true
                        $script:indexMetrics = Require-Relay 'project.index.build' @{ project_id = $ProjectId } $true
                        $capabilities = Require-Relay 'project.capabilities' @{ project_id = $ProjectId }
                        $script:projectReady = ($capabilities.index_status -eq 'ready' -and $script:indexMetrics.generation -ge 1)
                        $status = if ($script:projectReady) { 'passed' } else { 'failed' }
                        $reason = if ($script:projectReady) { 'INDEX_READY' } else { 'INDEX_NOT_READY' }
                        $evidenceEligible = $true
                    }
                    'project.supported_host_resource' {
                        if ($SupportedHostTier -eq 'unknown') {
                            $status = 'untested'; $reason = 'SUPPORTED_HOST_TIER_NOT_DECLARED'; break
                        }
                        if (-not $script:projectReady) {
                            $status = 'blocked'; $reason = 'PROJECT_BASELINE_MISSING'; break
                        }
                        $hardware = Get-HostHardwareProbe
                        if (-not $hardware.available) {
                            $status = 'untested'; $reason = 'HOST_HARDWARE_PROBE_UNAVAILABLE'; break
                        }
                        if ($hardware.physical_cores -lt 4 -or $hardware.memory_bytes -lt 17179869184 -or
                            ($SupportedHostTier -eq 'recommended_candidate' -and
                                $hardware.memory_bytes -lt 34359738368)) {
                            $status = 'untested'; $reason = 'SUPPORTED_HOST_PROFILE_MISMATCH'; break
                        }
                        $foregroundProbe = Initialize-ForegroundProbe
                        $script:foregroundProbeAvailable = $foregroundProbe.available
                        $exchange = Invoke-Relay 'project.index.build' @{ project_id = $ProjectId } $true 1 $true
                        if (-not $exchange.ok) {
                            $status = 'failed'; $reason = Get-SafeCode $exchange.error_code
                            $evidenceEligible = $script:hadValidatedResponse
                            break
                        }
                        $capture = $exchange.resource_capture
                        if ($null -eq $capture -or $capture.sample_count -lt 2 -or
                            $capture.daemon_peak_working_set_bytes -le 0 -or
                            $capture.cli_peak_working_set_bytes -le 0 -or
                            $exchange.result.generation -lt 1) {
                            $status = 'failed'; $reason = 'HOST_RESOURCE_SAMPLE_INCOMPLETE';
                            $evidenceEligible = $true; break
                        }
                        $memoryClass = if ($hardware.memory_bytes -ge 34359738368) { 'at_least32_gib' }
                            elseif ($hardware.memory_bytes -ge 17179869184) { 'at_least16_gib' }
                            else { 'below16_gib' }
                        $cpuClass = if ($hardware.physical_cores -ge 4) {
                            'at_least_four_physical_cores'
                        } else { 'below_four_physical_cores' }
                        $foregroundObservation = if ($capture.foreground_observed_samples -eq 0) {
                            'unavailable'
                        } elseif ($capture.foreground_creator_samples -gt 0) { 'seen' } else { 'not_seen' }
                        $script:hostResourceMetric = [ordered]@{
                            host_tier_declaration = $SupportedHostTier
                            support_tier_budget_status = 'untested'
                            physical_core_count = [int]$hardware.physical_cores
                            logical_processor_count = [int]$hardware.logical_processors
                            physical_memory_bytes = [long]$hardware.memory_bytes
                            observed_memory_class = $memoryClass
                            observed_cpu_class = $cpuClass
                            index_command_elapsed_ms = [long]$capture.index_command_elapsed_ms
                            daemon_cpu_ms = [long]$capture.daemon_cpu_ms
                            cli_cpu_ms = [long]$capture.cli_cpu_ms
                            daemon_peak_working_set_bytes = [long]$capture.daemon_peak_working_set_bytes
                            cli_peak_working_set_bytes = [long]$capture.cli_peak_working_set_bytes
                            sample_count = [int]$capture.sample_count
                            sample_interval_ms = 100
                            sampling_overhead_ms = [long]$capture.sampling_overhead_ms
                            host_probe_overhead_ms = [long]$hardware.overhead_ms
                            foreground_probe_overhead_ms = [long]$foregroundProbe.overhead_ms
                            foreground_observation = $foregroundObservation
                            foreground_creator_samples = [int]$capture.foreground_creator_samples
                            foreground_interference_status = 'untested'
                        }
                        $status = 'passed'; $reason = 'LOCAL_HOST_RESOURCE_MEASURED'
                        $evidenceEligible = $true
                    }
                    'project.foreground_interference' {
                        $status = 'untested'
                        $reason = 'PAIRED_PROLONGED_CREATOR_TRACE_REQUIRED'
                    }
                    'project.lifecycle' {
                        if (-not $ProjectRoot) { $status = 'untested'; $reason = 'PROJECT_ROOT_NOT_SUPPLIED'; break }
                        $null = Require-Relay 'project.import' @{ id = $lifeId; name = 'Lifecycle Validation Project'; root_path = $ProjectRoot } $true
                        $archived = Require-Relay 'project.archive' @{ project_id = $lifeId } $true
                        $restored = Require-Relay 'project.restore' @{ project_id = $lifeId } $true
                        $removalPlan = Require-Relay 'project.removal.plan' @{ project_id = $lifeId } $true
                        if ($removalPlan.state -ne 'pending' -or -not $removalPlan.approval_id) {
                            $status = 'failed'; $reason = 'REMOVAL_PLAN_INVALID'; break
                        }
                        $removalRead = Require-Relay 'project.removal.get' @{
                            project_id = $lifeId; approval_id = [string]$removalPlan.approval_id
                        }
                        if ($removalRead.state -ne 'pending' -or $removalRead.approval_id -ne $removalPlan.approval_id) {
                            $status = 'failed'; $reason = 'REMOVAL_REVIEW_INVALID'; break
                        }
                        $decision = Invoke-Relay 'project.removal.decide' @{
                            project_id = $lifeId; approval_id = [string]$removalPlan.approval_id; decision = 'approve'
                        } $true
                        if (-not $decision.ok) {
                            $status = if ($decision.error_code -in @('APPROVAL_DENIED','APPROVAL_REQUIRED','POLICY_DENIED')) { 'untested' } else { 'failed' }
                            $reason = if ($status -eq 'untested') { 'TRUSTED_APPROVAL_UNAVAILABLE' } else { Get-SafeCode $decision.error_code }
                            break
                        }
                        if ($decision.result.state -ne 'approved_pending_execution') {
                            $status = 'failed'; $reason = 'REMOVAL_DECISION_INVALID'; break
                        }
                        $removed = Require-Relay 'project.remove' @{
                            project_id = $lifeId; approval_id = [string]$removalPlan.approval_id
                        } $true 2
                        $listing = Require-Relay 'project.list' @{ include_inactive = $true }
                        $present = @($listing.projects | Where-Object { $_.id -eq $lifeId -and $_.lifecycle_state -eq 'removed' }).Count -eq 1
                        $status = if ($archived.lifecycle_state -eq 'archived' -and $restored.lifecycle_state -eq 'active' -and $removed.state -eq 'executed' -and $removed.executed -eq $true -and $present) { 'passed' } else { 'failed' }
                        $reason = if ($status -eq 'passed') { 'LOCAL_APPROVAL_STATE_VERIFIED' } else { 'LIFECYCLE_MISMATCH' }
                        $evidenceEligible = $true
                    }
                    'context.result_storage' {
                        if (-not $script:projectReady) { $status = 'blocked'; $reason = 'PROJECT_BASELINE_MISSING'; break }
                        $stored = Require-Relay 'result.put' @{ project_id = $ProjectId; kind = 'RELAY_VALIDATION_INDEX'; payload = @{ generation = $script:indexMetrics.generation; file_count = $script:indexMetrics.file_count } } $true
                        $script:firstResultId = [string]$stored.id
                        $listing = Require-Relay 'result.list' @{ project_id = $ProjectId; limit = 100 }
                        $found = @($listing.results | Where-Object { $_.id -eq $script:firstResultId }).Count -eq 1
                        $status = if ($found) { 'passed' } else { 'failed' }
                        $reason = if ($found) { 'RESULT_STORED' } else { 'RESULT_NOT_LISTED' }
                        $evidenceEligible = $true
                    }
                    'context.compile_multi_result' {
                        if (-not $script:firstResultId) { $status = 'blocked'; $reason = 'RESULT_SOURCE_MISSING'; break }
                        $stored = Require-Relay 'result.put' @{ project_id = $ProjectId; kind = 'RELAY_VALIDATION_SECOND'; payload = @{ observed = $true; generation = $script:indexMetrics.generation } } $true
                        $script:secondResultId = [string]$stored.id
                        $compiled = Require-Relay 'context.compile' @{ project_id = $ProjectId; result_ids = @($script:firstResultId, $script:secondResultId); max_bytes = 4096 }
                        $status = if (@($compiled.sources).Count -eq 2) { 'passed' } else { 'failed' }
                        $reason = if ($status -eq 'passed') { 'MULTI_RESULT_COMPILED' } else { 'CONTEXT_SOURCES_MISSING' }
                        $evidenceEligible = $true
                    }
                    'context.task_snapshot' {
                        if (-not $script:projectReady -or -not $script:firstResultId -or -not $script:secondResultId) {
                            $status = 'blocked'; $reason = 'RESULT_SOURCE_MISSING'; break
                        }
                        $planRecord = Require-Relay 'project.removal.plan' @{ project_id = $ProjectId } $true
                        $decisionRecord = Require-Relay 'project.removal.decide' @{
                            project_id = $ProjectId; approval_id = [string]$planRecord.approval_id; decision = 'reject'
                        } $true
                        $task = Require-Relay 'context.task.compile' @{
                            project_id = $ProjectId; task_kind = 'review'; max_bytes = 8192
                            result_ids = @($script:firstResultId, $script:secondResultId)
                            approval_ids = @([string]$planRecord.approval_id)
                            required_pointers = @(@{ result_id = $script:firstResultId; pointer = '/file_count' })
                            focus_terms = @('generation')
                        }
                        $hasFact = @($task.result_context.facts | Where-Object {
                            $_.result_id -eq $script:firstResultId -and $_.pointer -eq '/file_count'
                        }).Count -eq 1
                        $hasDecision = @($task.decision_evidence | Where-Object {
                            $_.record_id -eq $planRecord.approval_id -and $_.state -eq 'rejected' -and
                            $_.human_presence -eq 'unverified'
                        }).Count -eq 1
                        $status = if ($decisionRecord.state -eq 'rejected' -and $hasFact -and $hasDecision -and
                            $task.project_state.index_state -eq 'ready' -and
                            $task.result_currentness -eq 'unknown_without_project_generation_link') { 'passed' } else { 'failed' }
                        $reason = if ($status -eq 'passed') { 'TASK_CONTEXT_SNAPSHOT_VERIFIED' } else { 'TASK_CONTEXT_MISMATCH' }
                        $evidenceEligible = $true
                    }
                    'context.conflict_handling' {
                        if (-not $script:projectReady) { $status = 'blocked'; $reason = 'PROJECT_BASELINE_MISSING'; break }
                        $left = Require-Relay 'result.put' @{ project_id = $ProjectId; kind = 'RELAY_VALIDATION_CONFLICT'; payload = @{ state = 'left' } } $true
                        $right = Require-Relay 'result.put' @{ project_id = $ProjectId; kind = 'RELAY_VALIDATION_CONFLICT'; payload = @{ state = 'right' } } $true
                        $compiled = Require-Relay 'context.compile' @{ project_id = $ProjectId; result_ids = @([string]$left.id, [string]$right.id); max_bytes = 4096 }
                        $status = if ($compiled.conflict_count -ge 1) { 'passed' } else { 'failed' }
                        $reason = if ($status -eq 'passed') { 'CONFLICT_PRESERVED' } else { 'CONFLICT_NOT_REPORTED' }
                        $evidenceEligible = $true
                    }
                    'context.cost_benchmark' {
                        if (-not $script:firstResultId -or -not $script:secondResultId) {
                            $status = 'blocked'; $reason = 'RESULT_SOURCE_MISSING'; break
                        }
                        $sourceIds = @($script:firstResultId, $script:secondResultId)
                        $fullBytes = 0L
                        $fullElapsed = 0L
                        foreach ($sourceId in $sourceIds) {
                            $exchange = Invoke-Relay 'result.get' @{ result_id = $sourceId }
                            if (-not $exchange.ok) {
                                $script:lastFailureCode = Get-SafeCode $exchange.error_code
                                throw 'WORKFLOW_COMMAND_FAILED'
                            }
                            if ($exchange.result.id -ne $sourceId -or $null -eq $exchange.result.payload) {
                                $script:lastFailureCode = 'RESULT_SOURCE_INVALID'
                                throw 'WORKFLOW_COMMAND_FAILED'
                            }
                            $payloadJson = ConvertTo-Json -InputObject $exchange.result.payload -Depth 20 -Compress
                            $fullBytes += [long]$utf8.GetByteCount($payloadJson)
                            $fullElapsed += [long]$exchange.transport_metric.elapsed_ms
                        }
                        $exchange = Invoke-Relay 'context.compile' @{
                            project_id = $ProjectId; result_ids = $sourceIds; max_bytes = 4096
                        }
                        if (-not $exchange.ok) {
                            $script:lastFailureCode = Get-SafeCode $exchange.error_code
                            throw 'WORKFLOW_COMMAND_FAILED'
                        }
                        if (@($exchange.result.sources).Count -ne $sourceIds.Count -or $fullBytes -le 0) {
                            $script:lastFailureCode = 'CONTEXT_BENCHMARK_INVALID'
                            throw 'WORKFLOW_COMMAND_FAILED'
                        }
                        $compiledJson = ConvertTo-Json -InputObject $exchange.result -Depth 20 -Compress
                        $compiledBytes = [long]$utf8.GetByteCount($compiledJson)
                        if ($compiledBytes -le 0) {
                            $script:lastFailureCode = 'CONTEXT_BENCHMARK_INVALID'
                            throw 'WORKFLOW_COMMAND_FAILED'
                        }
                        $script:contextCostMetric = [ordered]@{
                            byte_scope = 'sum_of_stored_payload_json_vs_compiled_result_json'
                            source_count = $sourceIds.Count
                            full_payload_json_bytes = $fullBytes
                            compiled_context_json_bytes = $compiledBytes
                            compiled_to_full_ratio_milli = [long][Math]::Floor(($compiledBytes * 1000.0) / $fullBytes)
                            full_payload_elapsed_ms = $fullElapsed
                            compiled_context_elapsed_ms = [long]$exchange.transport_metric.elapsed_ms
                            token_estimate_status = 'not_measured'
                            model_answer_quality_status = 'untested'
                            remote_cost_status = 'untested'
                        }
                        $status = 'passed'; $reason = 'LOCAL_CONTEXT_BYTES_MEASURED'
                        $evidenceEligible = $true
                    }
                    'uefn.static_inspection' {
                        if (-not $script:projectReady) { $status = 'untested'; $reason = 'REAL_UEFN_PROJECT_NOT_SUPPLIED'; break }
                        $inspection = Invoke-Relay 'uefn.static.inspect' @{ project_id = $ProjectId }
                        $status = if ($inspection.ok) { 'untested' } else { 'failed' }
                        $reason = if (-not $inspection.ok) { Get-SafeCode $inspection.error_code }
                        elseif ($inspection.result.marker_state -eq 'present') {
                            'REAL_UEFN_PROJECT_UNVERIFIED'
                        } else { 'UEFN_STATIC_INSPECTION_UNAVAILABLE' }
                        $evidenceEligible = ($status -eq 'failed' -and $script:hadValidatedResponse)
                    }
                    'uefn.mcp_discovery' {
                        $discovery = Invoke-Relay 'uefn.mcp.toolsets' @{}
                        $status = if ($discovery.ok) { 'untested' } else { 'failed' }
                        $reason = if (-not $discovery.ok) { Get-SafeCode $discovery.error_code }
                        elseif ($discovery.result.state -eq 'discovered') {
                            'EDITOR_IDENTITY_UNVERIFIED'
                        } else { 'UEFN_MCP_UNAVAILABLE' }
                        $evidenceEligible = ($status -eq 'failed' -and $script:hadValidatedResponse)
                    }
                    'verse.imported_analysis' {
                        if (-not $script:projectReady) { $status = 'blocked'; $reason = 'PROJECT_BASELINE_MISSING'; break }
                        if (-not $VerseCapturePath) { $status = 'untested'; $reason = 'CAPTURE_NOT_SUPPLIED'; break }
                        $capture = Read-BoundedInput $VerseCapturePath 32768
                        $analysis = Require-Relay 'runtime.capture.analyze' @{
                            project_id = $ProjectId; session_id = ('SESSION-' + $runId); source_kind = 'imported_log';
                            source_version = 'imported'; capture_ref = ('CAPTURE-' + $runId); capture_text = $capture
                        }
                        $status = if ($analysis.live_uefn_status -eq 'untested') { 'passed' } else { 'failed' }
                        $reason = if ($status -eq 'passed') { 'IMPORTED_ANALYSIS_COMPLETE' } else { 'ANALYSIS_STATUS_INVALID' }
                        $evidenceEligible = $true
                    }
                    'verse.project_file_analysis' {
                        if (-not $script:projectReady) { $status = 'blocked'; $reason = 'PROJECT_BASELINE_MISSING'; break }
                        if (-not $VerseProjectRelativePath -or -not $VerseSessionId) {
                            $status = 'untested'; $reason = 'PROJECT_CAPTURE_INPUT_NOT_SUPPLIED'; break
                        }
                        $analysis = Require-Relay 'runtime.capture.file.analyze' @{
                            project_id = $ProjectId; session_id = $VerseSessionId;
                            relative_path = $VerseProjectRelativePath
                        }
                        $observed = $analysis.acquisition_kind -eq 'project_local_file' -and
                            $analysis.source_kind -eq 'imported_log' -and
                            $analysis.source_version -eq 'project-file-v1' -and
                            $analysis.live_uefn_status -eq 'untested' -and
                            $analysis.capture_sha256 -cmatch '^[0-9a-f]{64}$' -and
                            $analysis.capture_bytes -le 32768
                        $status = if ($observed) { 'passed' } else { 'failed' }
                        $reason = if ($observed) { 'PROJECT_FILE_ANALYSIS_COMPLETE' } else { 'PROJECT_FILE_ANALYSIS_INVALID' }
                        $evidenceEligible = $true
                    }
                    'assets.local_links' {
                        if (-not $script:projectReady) { $status = 'blocked'; $reason = 'PROJECT_BASELINE_MISSING'; break }
                        if (-not $AssetManifestPath) { $status = 'untested'; $reason = 'MANIFEST_NOT_SUPPLIED'; break }
                        $script:manifestText = Read-BoundedInput $AssetManifestPath 24576
                        $assetReport = Require-Relay 'assets.manifest.validate' @{ project_id = $ProjectId; manifest_json = $script:manifestText }
                        $status = if ($assetReport.local_checks_passed -eq $true) { 'passed' } else { 'failed' }
                        $reason = if ($status -eq 'passed') { 'ASSET_LINKS_VALID' } else { 'ASSET_LOCAL_FINDINGS' }
                        $evidenceEligible = $true
                    }
                    'assets.impact_analysis' {
                        if (-not $script:projectReady) { $status = 'blocked'; $reason = 'PROJECT_BASELINE_MISSING'; break }
                        if (-not $script:manifestText -or -not $ChangedPath) { $status = 'untested'; $reason = 'IMPACT_INPUT_NOT_SUPPLIED'; break }
                        $impact = Require-Relay 'assets.impact.analyze' @{
                            project_id = $ProjectId; manifest_json = $script:manifestText; changed_paths = @($ChangedPath)
                        }
                        $status = if ($impact.creator_app_execution -eq 'not_checked' -and $impact.changed_path_count -eq 1) { 'passed' } else { 'failed' }
                        $reason = if ($status -eq 'passed') { 'LOCAL_IMPACT_COMPUTED' } else { 'IMPACT_RESULT_INVALID' }
                        $evidenceEligible = $true
                    }
                    'krita.declared_formats' {
                        if (-not $script:projectReady) { $status = 'blocked'; $reason = 'PROJECT_BASELINE_MISSING'; break }
                        if (-not $script:manifestText) { $status = 'untested'; $reason = 'MANIFEST_NOT_SUPPLIED'; break }
                        $kritaReport = Require-Relay 'assets.krita.inspect' @{ project_id = $ProjectId; manifest_json = $script:manifestText }
                        $status = if ($kritaReport.native_workflow -eq 'untested' -and $kritaReport.generic_finding_count -eq 0 -and $kritaReport.krita_format_finding_count -eq 0) { 'passed' } else { 'failed' }
                        $reason = if ($status -eq 'passed') { 'KRITA_DECLARATIONS_VALID' } else { 'KRITA_DECLARATION_FINDINGS' }
                        $evidenceEligible = $true
                    }
                    'krita.recovery_candidate' {
                        $fixtureRoot = Join-Path $script:ownedFull 'krita-recovery-project'
                        $null = [System.IO.Directory]::CreateDirectory($fixtureRoot)
                        $source = Join-Path $fixtureRoot 'source.kra'
                        $output = Join-Path $fixtureRoot 'existing.png'
                        [System.IO.File]::WriteAllText($source, 'unverified KRA fixture', $utf8)
                        [byte[]]$png = @(137,80,78,71,13,10,26,10,0,0,0,13,73,72,68,82,0,0,0,1,0,0,0,1,0,0,0,0,0,0,0,0,0)
                        [System.IO.File]::WriteAllBytes($output, $png)
                        $beforeHash = (Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash
                        $fixtureProjectId = 'VAL-KRITA-' + [Guid]::NewGuid().ToString('N').Substring(0, 12)
                        $null = Require-Relay 'project.import' @{
                            id = $fixtureProjectId; name = 'Integrated Krita Recovery Fixture'; root_path = $fixtureRoot
                        } $true
                        $arguments = @{
                            project_id = $fixtureProjectId; source_path = 'source.kra'; export_path = 'existing.png'
                        }
                        $candidate = Require-Relay 'assets.krita.reconcile' $arguments $true
                        $replay = Require-Relay 'assets.krita.reconcile' $arguments $true
                        $saved = Require-Relay 'result.get' @{ result_id = [string]$candidate.result_id }
                        $afterHash = (Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash
                        $valid = $candidate.status -eq 'candidate' -and
                            $candidate.record_state -eq 'candidate_stored' -and
                            $candidate.native_origin -eq 'unverified' -and
                            $candidate.native_workflow_status -eq 'untested' -and
                            $candidate.result_id -eq $replay.result_id -and
                            $replay.record_replayed -eq $true -and
                            $saved.project_id -eq $fixtureProjectId -and
                            $saved.kind -eq 'ASSET_KRITA_RECOVERY_CANDIDATE' -and
                            $saved.payload.native_workflow_status -eq 'untested' -and
                            $beforeHash -eq $afterHash
                        $status = if ($valid) { 'passed' } else { 'failed' }
                        $reason = if ($valid) { 'RECOVERY_CANDIDATE_RECORDED' } else { 'RECOVERY_CANDIDATE_INVALID' }
                        $evidenceEligible = $true
                    }
                    'blender.mesh_validation' {
                        if (-not $script:projectReady) { $status = 'blocked'; $reason = 'PROJECT_BASELINE_MISSING'; break }
                        if (-not $BlendPath) { $status = 'untested'; $reason = 'BLEND_PATH_NOT_SUPPLIED'; break }
                        $mesh = Require-Relay 'assets.blender.mesh.validate' @{ project_id = $ProjectId; blend_path = $BlendPath }
                        $status = if ($mesh.status -eq 'passed' -and $mesh.native_workflow_status -eq 'checked') { 'passed' }
                            elseif ($mesh.status -eq 'unavailable') { 'untested' }
                            else { 'failed' }
                        $reason = if ($status -eq 'passed') { 'BOUNDED_MESH_CHECK_PASSED' }
                            elseif ($status -eq 'untested') { 'BOUNDED_MESH_CHECK_UNAVAILABLE' }
                            else { 'BOUNDED_MESH_CHECK_FAILED' }
                        $evidenceEligible = ($status -ne 'untested')
                    }
                    'automation.pause_resume' {
                        $paused = Require-Relay 'automation.pause' @{} $true
                        $resumed = Require-Relay 'automation.resume' @{} $true
                        $status = if ($paused.mode -eq 'paused' -and $resumed.mode -eq 'running') { 'passed' }
                            elseif ($paused.mode -eq 'unavailable' -or $resumed.mode -eq 'unavailable') { 'untested' }
                            else { 'failed' }
                        $reason = if ($status -eq 'passed') { 'AUTOMATION_CONTROL_VERIFIED' }
                            elseif ($status -eq 'untested') { 'AUTOMATION_UNAVAILABLE' }
                            else { 'AUTOMATION_CONTROL_MISMATCH' }
                        $evidenceEligible = ($status -ne 'untested')
                    }
                    'automation.check_plan' {
                        if (-not $script:projectReady) { $status = 'blocked'; $reason = 'PROJECT_BASELINE_MISSING'; break }
                        $catalogRead = Invoke-Relay 'project.check_catalog.get' @{ project_id = $ProjectId }
                        if (-not $catalogRead.ok) {
                            $status = if ($catalogRead.error_code -eq 'CHECK_CATALOG_MISSING') { 'untested' } else { 'failed' }
                            $reason = if ($status -eq 'untested') { 'CHECK_CATALOG_NOT_REGISTERED_IN_PRIVATE_HOST' } else { Get-SafeCode $catalogRead.error_code }
                            $evidenceEligible = ($status -eq 'failed' -and $script:hadValidatedResponse)
                            break
                        }
                        $planned = Invoke-Relay 'automation.checks.plan' @{
                            project_id = $ProjectId; after_generation = [long]$script:indexMetrics.generation
                        }
                        if (-not $planned.ok) {
                            $status = 'failed'; $reason = Get-SafeCode $planned.error_code
                            $evidenceEligible = $script:hadValidatedResponse
                            break
                        }
                        $checkPlan = $planned.result
                        $allUnrun = @($checkPlan.checks | Where-Object {
                            $_.status -ne 'planned_not_run' -or $null -ne $_.result_id
                        }).Count -eq 0
                        if ($checkPlan.project_id -ne $ProjectId -or
                            $checkPlan.catalog_revision -ne $catalogRead.result.revision -or
                            $checkPlan.index_generation -ne $script:indexMetrics.generation -or
                            -not $allUnrun) {
                            $status = 'failed'; $reason = 'CHECK_PLAN_CONTRACT_INVALID'; $evidenceEligible = $true
                        } else {
                            $status = 'untested'; $reason = 'AFFECTED_DELTA_NOT_EXERCISED'
                        }
                    }
                    'automation.declared_index_checks' {
                        if (-not $script:projectReady) { $status = 'blocked'; $reason = 'PROJECT_BASELINE_MISSING'; break }
                        $catalogRead = Invoke-Relay 'project.check_catalog.get' @{ project_id = $ProjectId }
                        if (-not $catalogRead.ok) {
                            $status = if ($catalogRead.error_code -eq 'CHECK_CATALOG_MISSING') { 'untested' } else { 'failed' }
                            $reason = if ($status -eq 'untested') { 'CHECK_CATALOG_NOT_REGISTERED_IN_PRIVATE_HOST' } else { Get-SafeCode $catalogRead.error_code }
                            $evidenceEligible = ($status -eq 'failed' -and $script:hadValidatedResponse)
                            break
                        }
                        $planned = Invoke-Relay 'automation.checks.plan' @{
                            project_id = $ProjectId; after_generation = [long]$script:indexMetrics.generation
                        }
                        if (-not $planned.ok) {
                            $status = 'failed'; $reason = Get-SafeCode $planned.error_code
                            $evidenceEligible = $script:hadValidatedResponse
                            break
                        }
                        $checkPlan = $planned.result
                        if ($checkPlan.mode -ne 'selective' -or @($checkPlan.checks).Count -eq 0) {
                            $status = 'untested'; $reason = 'DECLARED_CHECK_DELTA_NOT_AVAILABLE'
                            break
                        }
                        $executed = Invoke-Relay 'automation.checks.execute' @{
                            project_id = $ProjectId; after_generation = [long]$script:indexMetrics.generation;
                            plan_id = [string]$checkPlan.plan_id
                        } $true
                        if (-not $executed.ok) {
                            $status = 'failed'; $reason = Get-SafeCode $executed.error_code
                            $evidenceEligible = $script:hadValidatedResponse
                            break
                        }
                        $checkRun = $executed.result
                        $hasNativeClaim = @($checkRun.checks | Where-Object { $_.native_workflow_status -eq 'checked' }).Count -gt 0
                        $hasMissingResult = @($checkRun.checks | Where-Object {
                            $_.status -in @('passed', 'failed') -and -not $_.result_id
                        }).Count -gt 0
                        $countMatches = [int]$checkRun.selected_check_count -eq @($checkRun.checks).Count -and
                            [int]$checkRun.selected_check_count -eq ([int]$checkRun.passed_count + [int]$checkRun.failed_count + [int]$checkRun.untested_count)
                        if ($checkRun.project_id -ne $ProjectId -or $checkRun.plan_id -ne $checkPlan.plan_id -or
                            $checkRun.index_generation -ne $script:indexMetrics.generation -or -not $countMatches -or
                            $hasNativeClaim -or $hasMissingResult) {
                            $status = 'failed'; $reason = 'DECLARED_CHECK_RESULT_CONTRACT_INVALID'
                        } elseif ([int]$checkRun.untested_count -eq [int]$checkRun.selected_check_count) {
                            $status = 'untested'; $reason = 'NO_EXECUTABLE_INDEX_ASSERTIONS'
                        } else {
                            $status = 'passed'; $reason = 'DECLARED_INDEX_ASSERTIONS_RECORDED'
                        }
                        $evidenceEligible = ($status -ne 'untested')
                    }
                    'automation.direct_index_fixture' {
                        $fixtureRoot = Join-Path $script:ownedFull 'direct-check-project'
                        $null = [System.IO.Directory]::CreateDirectory($fixtureRoot)
                        $firstFile = Join-Path $fixtureRoot 'one.txt'
                        $secondFile = Join-Path $fixtureRoot 'two.txt'
                        [System.IO.File]::WriteAllText($firstFile, 'first-v1', $utf8)
                        [System.IO.File]::WriteAllText($secondFile, 'second-v1', $utf8)
                        $fixtureProjectId = 'VAL-CHECK-' + [Guid]::NewGuid().ToString('N').Substring(0, 12)
                        $null = Require-Relay 'project.import' @{
                            id = $fixtureProjectId; name = 'Integrated Direct Check Fixture'; root_path = $fixtureRoot
                        } $true
                        $baseline = Require-Relay 'project.index.build' @{ project_id = $fixtureProjectId } $true
                        $catalog = @{
                            format_version = 1
                            checks = @(
                                @{ id = 'check.one'; roots = @('one.txt'); leaves = @(); dependency_mode = 'direct'; assertion = @{ kind = 'indexed_file_present'; path = 'one.txt' } },
                                @{ id = 'check.two'; roots = @('two.txt'); leaves = @(); dependency_mode = 'direct'; assertion = @{ kind = 'indexed_file_present'; path = 'two.txt' } }
                            )
                        }
                        $registered = Require-Relay 'project.check_catalog.put' @{
                            project_id = $fixtureProjectId; expected_revision = 0; catalog = $catalog
                        } $true
                        $initial = Require-Relay 'automation.checks.plan' @{
                            project_id = $fixtureProjectId; after_generation = [long]$baseline.generation
                        }
                        if ($initial.mode -ne 'selective' -or $initial.coverage_basis -ne 'declared_direct_paths' -or
                            @($initial.checks).Count -ne 2 -or $registered.index_generation -ne $baseline.generation) {
                            $status = 'failed'; $reason = 'DIRECT_BASELINE_PLAN_INVALID'; $evidenceEligible = $true; break
                        }
                        $initialRun = Require-Relay 'automation.checks.execute' @{
                            project_id = $fixtureProjectId; after_generation = [long]$baseline.generation;
                            plan_id = [string]$initial.plan_id
                        } $true
                        if ($initialRun.passed_count -ne 2 -or $initialRun.failed_count -ne 0 -or
                            $initialRun.untested_count -ne 0 -or @($initialRun.checks | Where-Object { -not $_.result_id }).Count -gt 0) {
                            $status = 'failed'; $reason = 'DIRECT_BASELINE_RUN_INVALID'; $evidenceEligible = $true; break
                        }
                        [System.IO.File]::WriteAllText($firstFile, 'first-v2-changed', $utf8)
                        $reconciled = Require-Relay 'project.index.reconcile' @{
                            project_id = $fixtureProjectId; verify_content = $true
                        } $true
                        $changed = Require-Relay 'automation.checks.plan' @{
                            project_id = $fixtureProjectId; after_generation = [long]$baseline.generation
                        }
                        if ($reconciled.generation -le $baseline.generation -or $changed.mode -ne 'selective' -or
                            @($changed.checks).Count -ne 1 -or $changed.checks[0].check_id -ne 'check.one') {
                            $status = 'failed'; $reason = 'DIRECT_AFFECTED_PLAN_INVALID'; $evidenceEligible = $true; break
                        }
                        $changedRun = Require-Relay 'automation.checks.execute' @{
                            project_id = $fixtureProjectId; after_generation = [long]$baseline.generation;
                            plan_id = [string]$changed.plan_id
                        } $true
                        $saved = Require-Relay 'result.get' @{ result_id = [string]$changedRun.checks[0].result_id }
                        $valid = $changedRun.passed_count -eq 1 -and $changedRun.failed_count -eq 0 -and
                            $changedRun.untested_count -eq 0 -and $changedRun.checks[0].check_id -eq 'check.one' -and
                            $saved.project_id -eq $fixtureProjectId -and
                            $saved.payload.native_workflow_status -eq 'untested' -and
                            $saved.payload.index_generation -eq $changedRun.index_generation
                        $status = if ($valid) { 'passed' } else { 'failed' }
                        $reason = if ($valid) { 'DIRECT_INDEX_CHECKS_VERIFIED' } else { 'DIRECT_INDEX_RUN_INVALID' }
                        $evidenceEligible = $true
                    }
                    'gateway.local_discovery_overhead' {
                        $cliDiscovery = Invoke-Relay 'registry.list' @{ surface = 'ai'; prefix = 'registry.'; limit = 8 }
                        if (-not $cliDiscovery.ok) {
                            $status = 'failed'; $reason = Get-SafeCode $cliDiscovery.error_code
                            $evidenceEligible = $script:hadValidatedResponse
                            break
                        }
                        $null = $script:transportMetrics.Add($cliDiscovery.transport_metric)
                        $mcpDiscovery = Invoke-LocalMcpRead
                        if ($mcpDiscovery.transport_metric) {
                            $null = $script:transportMetrics.Add($mcpDiscovery.transport_metric)
                        }
                        if (-not $mcpDiscovery.ok) {
                            $status = if ($mcpDiscovery.attempted) { 'failed' } else { 'untested' }
                            $reason = $mcpDiscovery.reason
                            $evidenceEligible = $mcpDiscovery.attempted
                            break
                        }
                        $cliIds = @($cliDiscovery.result.commands | ForEach-Object { [string]$_.id })
                        $mcpIds = @($mcpDiscovery.result.commands | ForEach-Object { [string]$_.id })
                        $same = $cliIds.Count -eq $mcpIds.Count -and
                            (($cliIds -join ',') -ceq ($mcpIds -join ','))
                        $status = if ($same) { 'passed' } else { 'failed' }
                        $reason = if ($same) { 'LOCAL_DISCOVERY_MEASURED' } else { 'LOCAL_DISCOVERY_MISMATCH' }
                        $evidenceEligible = $true
                    }
                    'gateway.local_result_context' {
                        if (-not $script:firstResultId) {
                            $status = 'blocked'; $reason = 'RESULT_SOURCE_MISSING'; break
                        }
                        $listed = Invoke-LocalMcpRead -ToolName 'relay_result_list' -Arguments @{
                            project_id = $ProjectId; limit = 20
                        }
                        if (-not $listed.ok) {
                            $status = if ($listed.attempted) { 'failed' } else { 'untested' }
                            $reason = $listed.reason; $evidenceEligible = $listed.attempted; break
                        }
                        $described = Invoke-LocalMcpRead -ToolName 'relay_result_describe' -Arguments @{
                            project_id = $ProjectId; result_id = $script:firstResultId
                        }
                        if (-not $described.ok) {
                            $status = if ($described.attempted) { 'failed' } else { 'untested' }
                            $reason = $described.reason; $evidenceEligible = $described.attempted; break
                        }
                        $read = Invoke-LocalMcpRead -ToolName 'relay_result_context' -Arguments @{
                            project_id = $ProjectId; result_id = $script:firstResultId;
                            max_bytes = 4096; required_pointers = @('/file_count')
                        }
                        if (-not $read.ok) {
                            $status = if ($read.attempted) { 'failed' } else { 'untested' }
                            $reason = $read.reason; $evidenceEligible = $read.attempted; break
                        }
                        $null = $script:transportMetrics.Add([ordered]@{
                            path_id = 'local_mcp_http'; byte_scope = 'application_json'
                            request_bytes = [long]$listed.transport_metric.request_bytes + [long]$described.transport_metric.request_bytes + [long]$read.transport_metric.request_bytes
                            response_bytes = [long]$listed.transport_metric.response_bytes + [long]$described.transport_metric.response_bytes + [long]$read.transport_metric.response_bytes
                            elapsed_ms = [long]$listed.transport_metric.elapsed_ms + [long]$described.transport_metric.elapsed_ms + [long]$read.transport_metric.elapsed_ms
                        })
                        $matched = @($listed.result.results).Count -gt 0 -and
                            $described.result.id -eq $script:firstResultId -and
                            @($read.result.facts | Where-Object {
                            $_.pointer -eq '/file_count' -and $_.value -eq $script:indexMetrics.file_count
                        }).Count -eq 1
                        $status = if ($matched) { 'passed' } else { 'failed' }
                        $reason = if ($matched) { 'LOCAL_RESULT_CONTEXT_READ' } else { 'LOCAL_RESULT_CONTEXT_MISMATCH' }
                        $evidenceEligible = $true
                    }
                    'report.evidence_capture' {
                        $summary = Require-Relay 'diagnostics.summary' @{}
                        $recentJson = ConvertTo-Json -InputObject @($summary.recent) -Depth 8 -Compress
                        $recentValid = $summary.recent -is [array] -and @($summary.recent).Count -le 12 -and
                            $recentJson.Length -le 4096 -and
                            -not $recentJson.Contains($script:ownedFull, [StringComparison]::OrdinalIgnoreCase) -and
                            (-not $ProjectRoot -or -not $recentJson.Contains($ProjectRoot, [StringComparison]::OrdinalIgnoreCase))
                        $hashesValid = $true
                        foreach ($entry in $script:journal) {
                            if ((Get-TextHash $entry.entry_json) -ne $entry.entry_sha256) { $hashesValid = $false; break }
                        }
                        $hasResource = @($script:results | Where-Object { $_.resource_use -and $_.resource_use.peak_rss_bytes }).Count -gt 0
                        $status = if ($summary.available -eq $true -and $recentValid -and $hashesValid -and $hasResource) { 'passed' } else { 'failed' }
                        $reason = if ($status -eq 'passed') { 'SAFE_EVIDENCE_CAPTURED' } else { 'EVIDENCE_CAPTURE_INCOMPLETE' }
                        $evidenceEligible = $true
                    }
                    default { }
                }
            }
        } catch {
            $status = 'failed'
            $reason = Get-SafeCode $script:lastFailureCode 'RUNNER_EXCEPTION'
            $evidenceEligible = $script:hadValidatedResponse -eq $true
        }
        $null = $script:results.Add((Complete-Workflow $workflow $status $reason $evidenceEligible))
    }

    $completedUnixMs = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    $counts = [ordered]@{ passed = 0; failed = 0; blocked = 0; untested = 0 }
    foreach ($result in $script:results) { $counts[$result.status]++ }
    $overall = if ($counts.failed -gt 0) { 'failed' } elseif ($counts.blocked -gt 0) { 'blocked' } elseif ($counts.untested -gt 0) { 'untested' } else { 'passed' }
    $report = [ordered]@{
        schema_version = 1
        run_id = $runId
        relay_version = $script:relayVersion
        started_unix_ms = $startedUnixMs
        completed_unix_ms = $completedUnixMs
        duration_ms = [long]($completedUnixMs - $startedUnixMs)
        overall_status = $overall
        counts = $counts
        workflows = @($script:results.ToArray())
    }
    Write-ReservedJson $journalFull ([ordered]@{ schema_version = 1; run_id = $runId; entries = @($script:journal.ToArray()) })
    Write-ReservedJson $outputFull $report
    Write-Output $outputFull
} catch {
    # Preserve a machine-readable failure artifact even when orchestration fails.
    $completedUnixMs = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    $resultIds = @{}
    foreach ($result in $script:results) { $resultIds[$result.workflow_id] = $true }
    $failureAssigned = $false
    foreach ($workflow in $plan.workflows) {
        if ($resultIds.ContainsKey($workflow.workflow_id)) { continue }
        $script:activeCommands = New-Object 'System.Collections.Generic.List[object]'
        $script:transportMetrics = New-Object 'System.Collections.Generic.List[object]'
        $script:contextCostMetric = $null
        $script:hostResourceMetric = $null
        $script:workflowStarted = $completedUnixMs
        $canRecordFailure = @($workflow.requires).Count -eq 0
        $status = if (-not $failureAssigned -and $canRecordFailure) { 'failed' } elseif ($workflow.execution -eq 'manual' -or -not $canRecordFailure) { 'untested' } else { 'blocked' }
        $reason = if ($status -eq 'failed') { 'RUNNER_INTERRUPTED' } else { 'NOT_EXECUTED' }
        $null = $script:results.Add((Complete-Workflow $workflow $status $reason $false))
        if ($status -eq 'failed') { $failureAssigned = $true }
    }
    if (-not $failureAssigned -and $script:results.Count -gt 0) {
        $script:results[$script:results.Count - 1].status = 'failed'
        $script:results[$script:results.Count - 1].reason_code = 'REPORT_WRITE_FAILED'
    }
    $counts = [ordered]@{ passed = 0; failed = 0; blocked = 0; untested = 0 }
    foreach ($result in $script:results) { $counts[$result.status]++ }
    $report = [ordered]@{
        schema_version = 1; run_id = $runId; relay_version = $script:relayVersion
        started_unix_ms = $startedUnixMs; completed_unix_ms = $completedUnixMs
        duration_ms = [long]($completedUnixMs - $startedUnixMs); overall_status = 'failed'
        counts = $counts; workflows = @($script:results.ToArray())
    }
    try { Write-ReservedJson $journalFull ([ordered]@{ schema_version = 1; run_id = $runId; entries = @($script:journal.ToArray()) }) } catch { }
    try { Write-ReservedJson $outputFull $report } catch { }
    Write-Output $outputFull
} finally {
    if ($script:daemon) {
        if ($script:daemonReady -and -not $script:daemon.HasExited) {
            try { $null = Invoke-Relay 'system.shutdown' @{} } catch { }
            try { $null = $script:daemon.WaitForExit(5000) } catch { }
        }
        if (-not $script:daemon.HasExited) { try { $script:daemon.Kill() } catch { } }
        try { $null = $script:daemon.WaitForExit(5000) } catch { }
        $script:daemon.Dispose()
    }
    if ($null -eq $previousState) { Remove-Item Env:RELAY_STATE_DIR -ErrorAction SilentlyContinue }
    else { $env:RELAY_STATE_DIR = $previousState }
    if ($null -eq $previousInstance) { Remove-Item Env:RELAY_INSTANCE -ErrorAction SilentlyContinue }
    else { $env:RELAY_INSTANCE = $previousInstance }
    if (Test-Path -LiteralPath $ownedFull -PathType Container) {
        if (Test-OwnedTempTarget $ownedFull $true) {
            try { Remove-Item -LiteralPath $ownedFull -Recurse -Force -ErrorAction Stop } catch { }
        }
    }
}
