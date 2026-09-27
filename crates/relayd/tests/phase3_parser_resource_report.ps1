param(
    [ValidateRange(1, 20)]
    [int]$Runs = 3,
    [string]$OutputPath
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path

function Get-SourceFingerprint {
    $files = @(
        Get-ChildItem (Join-Path $repoRoot 'crates') -Recurse -File |
            Where-Object { $_.Extension -eq '.rs' -or $_.Name -eq 'Cargo.toml' }
        Get-Item (Join-Path $repoRoot 'Cargo.toml')
        Get-Item (Join-Path $repoRoot 'Cargo.lock')
    )
    $lines = @($files | Sort-Object FullName | ForEach-Object {
        $relative = $_.FullName.Substring($repoRoot.Length + 1).Replace('\', '/')
        "$relative=$((Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash)"
    })
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [System.Text.Encoding]::UTF8.GetBytes(($lines -join "`n"))
        return [Convert]::ToHexString($sha.ComputeHash($bytes)).ToLowerInvariant()
    }
    finally {
        $sha.Dispose()
    }
}

function Get-Summary {
    param([double[]]$Values)
    $sorted = @($Values | Sort-Object)
    return [ordered]@{
        count = $sorted.Count
        min = $sorted[0]
        median = $sorted[[Math]::Ceiling($sorted.Count * 0.5) - 1]
        max = $sorted[-1]
    }
}

$processor = Get-CimInstance Win32_Processor
$computer = Get-CimInstance Win32_ComputerSystem
$hostClass = [ordered]@{
    os = (Get-CimInstance Win32_OperatingSystem).Caption
    cpu_model = ($processor | Select-Object -First 1).Name.Trim()
    physical_cores = [int](($processor | Measure-Object NumberOfCores -Sum).Sum)
    logical_processors = [int](($processor | Measure-Object NumberOfLogicalProcessors -Sum).Sum)
    ram_bytes = [uint64]$computer.TotalPhysicalMemory
}

$samples = @()
Push-Location $repoRoot
try {
    & cargo build -q -p relay-adapter --bin relay-adapter-fixture
    if ($LASTEXITCODE -ne 0) { throw 'Could not build the synthetic parser worker' }
    $revision = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Could not identify the source revision' }
    $dirty = @(& git status --porcelain).Count -gt 0
    $fingerprint = Get-SourceFingerprint
    for ($run = 1; $run -le $Runs; $run++) {
        if ((Get-SourceFingerprint) -ne $fingerprint) {
            throw "Rust source changed before run $run; discard this timing set"
        }
        Write-Host "Installed-parser resource run $run of $Runs"
        $output = (& cargo test -q -p relayd --test phase3_parser_resource installed_parser_idle_and_reparse_cost -- --ignored --nocapture 2>&1 | Out-String)
        if ($LASTEXITCODE -ne 0) {
            throw "Installed-parser resource fixture failed on run $run`n$output"
        }
        if ((Get-SourceFingerprint) -ne $fingerprint) {
            throw "Rust source changed during run $run; discard this timing set"
        }
        $match = [regex]::Match($output, '(?m)^PHASE3_PARSER_RESOURCE_METRICS=(\{[^\r\n]+\})')
        if (-not $match.Success) { throw "Parser resource metrics missing on run $run`n$output" }
        $sample = $match.Groups[1].Value | ConvertFrom-Json
        if ($sample.parser_grants -ne 1 -or $sample.source_files -ne 1 -or
            @($sample.reparse_publish_ms_after_reconcile).Count -ne 5 -or
            $sample.idle_sample_ms -ne 5000) {
            throw "Unexpected parser resource work counts on run $run"
        }
        $samples += $sample
    }
}
finally {
    Pop-Location
}

$summary = [ordered]@{}
foreach ($name in @('initial_publish_ms_after_reconcile', 'unbound_idle_rss_bytes',
        'unbound_idle_cpu_ms', 'installed_idle_rss_bytes', 'installed_idle_cpu_ms')) {
    $summary[$name] = Get-Summary ([double[]]@($samples | ForEach-Object { $_.$name }))
}
$summary['reparse_publish_ms_after_reconcile'] = Get-Summary ([double[]]@(
    $samples | ForEach-Object { $_.reparse_publish_ms_after_reconcile }
))

$report = [ordered]@{
    schema_version = 1
    captured_utc = (Get-Date).ToUniversalTime().ToString('o')
    source_revision = $revision
    rust_source_sha256 = $fingerprint
    working_tree_dirty = $dirty
    build_profile = 'debug'
    run_count = $Runs
    host_class = $hostClass
    caveats = @(
        'One 28-byte synthetic JSON source and two target files; one installed digest-pinned sandboxed parser grant.',
        'Publication latency begins after reconciliation and includes idle scheduling, sandbox worker launch, Core commit, and client polling.',
        'Five-second CPU/RSS windows are short; unbound and installed samples are sequential daemon processes with different histories.',
        'The debug fixture accelerates user-idle eligibility to one millisecond; production uses thirty seconds.',
        'No creator application, minimum/recommended hardware tier, large-file parser input, or long-run poll measurement.'
    )
    summary = $summary
    samples = $samples
}
$json = $report | ConvertTo-Json -Depth 10
if ($OutputPath) {
    $resolved = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputPath)
    [System.IO.File]::WriteAllText($resolved, $json + [Environment]::NewLine)
    Write-Host "Report written to $resolved"
}
$json
