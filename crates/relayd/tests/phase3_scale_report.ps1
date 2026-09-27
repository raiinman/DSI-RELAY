param(
    [ValidateRange(1, 100)]
    [int]$Runs = 5,
    [string]$OutputPath
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$metricNames = @(
    'watcher_one_file_stale_wall_ms',
    'post_hint_reconcile_ms',
    'full_content_verify_ms',
    'idle_rss_bytes',
    'idle_cpu_delta_ms',
    'sqlite_main_wal_shm_bytes'
)

function Get-NearestRank {
    param([double[]]$Values, [double]$Percentile)
    $sorted = @($Values | Sort-Object)
    $index = [Math]::Max(0, [Math]::Ceiling($Percentile * $sorted.Count) - 1)
    return $sorted[$index]
}

function Get-Summary {
    param([double[]]$Values)
    $sorted = @($Values | Sort-Object)
    return [ordered]@{
        count = $sorted.Count
        min = $sorted[0]
        median = Get-NearestRank $sorted 0.50
        p95 = Get-NearestRank $sorted 0.95
        max = $sorted[-1]
    }
}

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
    $sourceRevision = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Could not identify the source revision' }
    $workingTreeDirty = @(& git status --porcelain).Count -gt 0
    $sourceFingerprint = Get-SourceFingerprint
    for ($run = 1; $run -le $Runs; $run++) {
        if ((Get-SourceFingerprint) -ne $sourceFingerprint) {
            throw "Rust source changed before run $run; discard this timing set"
        }
        Write-Host "Scale fixture run $run of $Runs"
        $output = (& cargo test -p relayd --test phase3_watcher benchmark_two_watched_projects_at_fifteen_thousand_files -- --ignored --nocapture 2>&1 | Out-String)
        if ($LASTEXITCODE -ne 0) {
            throw "Scale fixture failed on run $run`n$output"
        }
        if ((Get-SourceFingerprint) -ne $sourceFingerprint) {
            throw "Rust source changed during run $run; discard this timing set"
        }
        $match = [regex]::Match($output, '(?m)^PHASE3_WATCHER_SCALE_METRICS=(\{[^\r\n]+\})')
        if (-not $match.Success) {
            throw "Scale fixture did not emit its metrics on run $run`n$output"
        }
        $sample = $match.Groups[1].Value | ConvertFrom-Json
        if ($sample.project_count -ne 2 -or $sample.total_fixture_files -ne 15000 -or
            $sample.post_hint_files_hashed -ne 0 -or $sample.full_content_files_hashed -ne 7500 -or
            @($sample.attachment_verify_ms).Count -ne 2 -or @($sample.metadata_reconcile_ms).Count -ne 2) {
            throw "Scale fixture emitted unexpected work counts on run $run"
        }
        $samples += $sample
    }
}
finally {
    Pop-Location
}

$summary = [ordered]@{}
foreach ($name in $metricNames) {
    $values = [double[]]@($samples | ForEach-Object { $_.$name })
    $summary[$name] = Get-Summary $values
}
foreach ($name in @('baseline_ms', 'attachment_verify_ms', 'metadata_reconcile_ms')) {
    $values = [double[]]@($samples | ForEach-Object { $_.$name[0]; $_.$name[1] })
    $summary[$name] = Get-Summary $values
}

$report = [ordered]@{
    schema_version = 1
    captured_utc = (Get-Date).ToUniversalTime().ToString('o')
    source_revision = $sourceRevision
    rust_source_sha256 = $sourceFingerprint
    working_tree_dirty = $workingTreeDirty
    build_profile = 'debug'
    source_fixture = 'benchmark_two_watched_projects_at_fifteen_thousand_files'
    run_count = $Runs
    host_class = $hostClass
    caveats = @(
        'Two 7500-file generic small-text projects per run; the fixture creates fresh roots and state.',
        'The one-file wall clock includes OS notification, batching, command polling, and scheduling.',
        'Idle CPU is a one-second process sample per run, not a long-run power or wakeup measure.',
        'No creator application foreground latency or minimum/recommended hardware tier is measured.'
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
