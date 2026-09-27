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
    'host.health', 'project.import_index', 'project.lifecycle', 'context.result_storage',
    'context.compile_multi_result', 'context.conflict_handling', 'context.cost_benchmark',
    'dashboard.browser', 'dashboard.accessibility',
    'uefn.static_inspection', 'uefn.mcp_discovery', 'uefn.spawn_audit', 'verse.imported_analysis', 'verse.project_file_analysis',
    'uefn.live_runtime', 'uefn.live_capture_assertions', 'assets.local_links', 'assets.impact_analysis',
    'krita.declared_formats', 'blender.mesh_validation', 'blender.native_mesh', 'krita.native_export',
    'gateway.local_discovery_overhead', 'gateway.remote_client', 'skills.local_client', 'remote.chatgpt_client',
    'onboarding.clean_account', 'onboarding.recovery', 'automation.pause_resume', 'automation.check_plan', 'hardware.minimum_pc',
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
    if (-not $Process) { return @{ peak_rss_bytes = $null; cpu_ms = $null } }
    try {
        $Process.Refresh()
        return @{
            peak_rss_bytes = [long]$Process.PeakWorkingSet64
            cpu_ms = [long]$Process.TotalProcessorTime.TotalMilliseconds
        }
    } catch { return @{ peak_rss_bytes = $null; cpu_ms = $null } }
}

function Read-BoundedInput {
    param([string]$Path, [long]$MaximumBytes)
    $file = Get-Item -LiteralPath $Path
    if ($file.Length -gt $MaximumBytes) { throw 'INPUT_TOO_LARGE' }
    return [System.IO.File]::ReadAllText($file.FullName, $utf8)
}

function Invoke-Relay {
    param([string]$CommandId, [hashtable]$CommandArguments, [bool]$Keyed = $false)
    $requestId = 'VALID-' + [Guid]::NewGuid().ToString('N')
    $request = [ordered]@{
        request_id = $requestId
        command = $CommandId
        command_version = 1
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
        $child.StandardInput.WriteLine($inputJson)
        $child.StandardInput.Close()
        if (-not $child.WaitForExit($CommandTimeoutMs)) {
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
    $daemonCpu = if ($null -ne $beforeDaemon.cpu_ms -and $null -ne $afterDaemon.cpu_ms) {
        [Math]::Max(0, ($afterDaemon.cpu_ms - $beforeDaemon.cpu_ms))
    } else { 0 }
    $cliCpu = if ($null -ne $cliSample.cpu_ms) { $cliSample.cpu_ms } else { 0 }
    $peak = [Math]::Max([long]$cliSample.peak_rss_bytes, [long]$afterDaemon.peak_rss_bytes)
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
            request_bytes = $requestBytes; response_bytes = $responseBytes; elapsed_ms = [long]$duration } }
}

function Invoke-LocalMcpDiscovery {
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
                name = 'relay_registry_list'; arguments = [ordered]@{ prefix = 'registry.'; limit = 8 }
                _meta = [ordered]@{
                    'io.modelcontextprotocol/protocolVersion' = '2026-07-28'
                    'io.modelcontextprotocol/clientInfo' = [ordered]@{ name = 'relay-validation'; version = '1' }
                    'io.modelcontextprotocol/clientCapabilities' = @{}
                }
            }
        } | ConvertTo-Json -Depth 12 -Compress
        $requestBytes = [long]$utf8.GetByteCount($body)
        if ($requestBytes -gt 8192) { throw 'MCP_REQUEST_TOO_LARGE' }
        $request = "POST /mcp HTTP/1.1`r`nHost: 127.0.0.1:8765`r`nAuthorization: Bearer $($credential.token)`r`nContent-Type: application/json`r`nAccept: application/json, text/event-stream`r`nMCP-Protocol-Version: 2026-07-28`r`nMcp-Method: tools/call`r`nMcp-Name: relay_registry_list`r`nContent-Length: $requestBytes`r`nConnection: close`r`n`r`n$body"
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
        if ($result.jsonrpc -ne '2.0' -or $result.id -ne 1 -or $result.error -or
            $result.result.isError -eq $true -or $null -eq $result.result.structuredContent.commands) {
            throw 'MCP_RESPONSE_INVALID'
        }
        $reason = 'LOCAL_DISCOVERY_MEASURED'
        return [pscustomobject]@{ ok = $true; attempted = $true; reason = $reason; result = $result.result.structuredContent;
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
                command_id = 'registry.list'; ok = ($reason -eq 'LOCAL_DISCOVERY_MEASURED');
                error_code = if ($reason -eq 'LOCAL_DISCOVERY_MEASURED') { $null } else { $reason };
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
    return $workflowResult
}

function Require-Relay {
    param([string]$CommandId, [hashtable]$CommandArguments, [bool]$Keyed = $false)
    $exchange = Invoke-Relay $CommandId $CommandArguments $Keyed
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
                    'project.lifecycle' {
                        if (-not $ProjectRoot) { $status = 'untested'; $reason = 'PROJECT_ROOT_NOT_SUPPLIED'; break }
                        $null = Require-Relay 'project.import' @{ id = $lifeId; name = 'Lifecycle Validation Project'; root_path = $ProjectRoot } $true
                        $archived = Require-Relay 'project.archive' @{ project_id = $lifeId } $true
                        $restored = Require-Relay 'project.restore' @{ project_id = $lifeId } $true
                        $removed = Require-Relay 'project.remove' @{ project_id = $lifeId; confirm_project_id = $lifeId } $true
                        $listing = Require-Relay 'project.list' @{ include_inactive = $true }
                        $present = @($listing.projects | Where-Object { $_.id -eq $lifeId -and $_.lifecycle_state -eq 'removed' }).Count -eq 1
                        $status = if ($archived.lifecycle_state -eq 'archived' -and $restored.lifecycle_state -eq 'active' -and $removed.lifecycle_state -eq 'removed' -and $present) { 'passed' } else { 'failed' }
                        $reason = if ($status -eq 'passed') { 'LIFECYCLE_VERIFIED' } else { 'LIFECYCLE_MISMATCH' }
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
                    'context.conflict_handling' {
                        if (-not $script:projectReady) { $status = 'blocked'; $reason = 'PROJECT_BASELINE_MISSING'; break }
                        $left = Require-Relay 'result.put' @{ project_id = $ProjectId; kind = 'RELAY_VALIDATION_CONFLICT'; payload = @{ state = 'left' } } $true
                        $right = Require-Relay 'result.put' @{ project_id = $ProjectId; kind = 'RELAY_VALIDATION_CONFLICT'; payload = @{ state = 'right' } } $true
                        $compiled = Require-Relay 'context.compile' @{ project_id = $ProjectId; result_ids = @([string]$left.id, [string]$right.id); max_bytes = 4096 }
                        $status = if ($compiled.conflict_count -ge 1) { 'passed' } else { 'failed' }
                        $reason = if ($status -eq 'passed') { 'CONFLICT_PRESERVED' } else { 'CONFLICT_NOT_REPORTED' }
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
                    'gateway.local_discovery_overhead' {
                        $cliDiscovery = Invoke-Relay 'registry.list' @{ surface = 'ai'; prefix = 'registry.'; limit = 8 }
                        if (-not $cliDiscovery.ok) {
                            $status = 'failed'; $reason = Get-SafeCode $cliDiscovery.error_code
                            $evidenceEligible = $script:hadValidatedResponse
                            break
                        }
                        $null = $script:transportMetrics.Add($cliDiscovery.transport_metric)
                        $mcpDiscovery = Invoke-LocalMcpDiscovery
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
                    'report.evidence_capture' {
                        $summary = Require-Relay 'diagnostics.summary' @{}
                        $hashesValid = $true
                        foreach ($entry in $script:journal) {
                            if ((Get-TextHash $entry.entry_json) -ne $entry.entry_sha256) { $hashesValid = $false; break }
                        }
                        $hasResource = @($script:results | Where-Object { $_.resource_use -and $_.resource_use.peak_rss_bytes }).Count -gt 0
                        $status = if ($summary.available -eq $true -and $hashesValid -and $hasResource) { 'passed' } else { 'failed' }
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
