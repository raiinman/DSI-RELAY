param(
    [Parameter(Mandatory = $true)]
    [string]$Version,
    [Parameter(Mandatory = $true)]
    [string]$BundleOutputDirectory,
    [Parameter(Mandatory = $true)]
    [string]$ReviewDirectory
)

$ErrorActionPreference = 'Stop'
$repoRoot = [System.IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
$tempRoot = [System.IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\')
$outputRoot = [System.IO.Path]::GetFullPath($BundleOutputDirectory)
$reviewRoot = [System.IO.Path]::GetFullPath($ReviewDirectory)
$name = "relay-measurement-preview-$Version-windows-x64"
$bundle = Join-Path $outputRoot $name
$archive = Join-Path $outputRoot "$name.zip"
$checksum = "$archive.sha256"
$extractName = 'relay-preview-ci-extract-' + [Guid]::NewGuid().ToString('N')
$extractRoot = Join-Path $tempRoot $extractName
$reportPath = Join-Path $tempRoot ('relay-preview-ci-report-' + [Guid]::NewGuid().ToString('N') + '.json')

function Get-BenchmarkTempNames {
    return @(Get-ChildItem -LiteralPath $tempRoot -Directory -Filter 'relay-portable-benchmark-*' -ErrorAction Stop |
        Select-Object -ExpandProperty Name)
}

function Assert-NoPrivateIdentifiers([string]$Path, [bool]$Report = $false) {
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    $views = @(
        [System.Text.Encoding]::GetEncoding(28591).GetString($bytes),
        [System.Text.Encoding]::Unicode.GetString($bytes)
    )
    $needles = @($env:USERNAME, $env:COMPUTERNAME, $env:USERPROFILE) |
        Where-Object { $_ -and $_.Length -ge 4 } | Select-Object -Unique
    if ($Report) { $needles += @('C:\Users\', 'C:/Users/') }
    foreach ($view in $views) {
        foreach ($needle in $needles) {
            if ($view.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
                throw "Private host identifier found in $([System.IO.Path]::GetFileName($Path))"
            }
        }
        if ($view -match '(?i)(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})') {
            throw "Token-shaped value found in $([System.IO.Path]::GetFileName($Path))"
        }
    }
}

$tempBefore = @(Get-BenchmarkTempNames)
$daemonBefore = @(Get-Process -Name relayd -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
try {
    foreach ($required in @($bundle, $archive, $checksum)) {
        if (-not (Test-Path -LiteralPath $required)) { throw 'Preview bundle output is incomplete' }
    }
    $hashLine = (Get-Content -LiteralPath $checksum -Raw).Trim()
    if ($hashLine -notmatch '^([0-9a-f]{64})  (.+\.zip)$' -or $Matches[2] -ne "$name.zip") {
        throw 'Archive checksum file is malformed'
    }
    $expectedHash = $Matches[1]
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expectedHash) {
        throw 'Archive SHA-256 mismatch'
    }
    & (Join-Path $repoRoot 'release\Verify-MeasurementPreview.ps1') -BundleDirectory $bundle

    $manifest = Get-Content -LiteralPath (Join-Path $bundle 'bundle-manifest.json') -Raw | ConvertFrom-Json
    if ($manifest.status -ne 'review_only' -or -not $manifest.source_tree_clean -or
        $manifest.source_revision -ne $env:GITHUB_SHA -or
        $manifest.binary_build_mode -ne 'built_by_packager' -or
        $manifest.third_party_notice_review -ne 'technical_selection_recorded' -or
        $manifest.bundled_sqlite_provenance -ne 'source_hash_and_features_verified') {
        throw ('Bundle manifest failed committed-source or technical review checks: clean={0}, revision_match={1}, notices={2}, sqlite={3}' -f
            $manifest.source_tree_clean, ($manifest.source_revision -eq $env:GITHUB_SHA),
            $manifest.third_party_notice_review, $manifest.bundled_sqlite_provenance)
    }

    $null = New-Item -ItemType Directory -Path $extractRoot
    Expand-Archive -LiteralPath $archive -DestinationPath $extractRoot -ErrorAction Stop
    $extractedBundle = Join-Path $extractRoot $name
    & (Join-Path $repoRoot 'release\Verify-MeasurementPreview.ps1') -BundleDirectory $extractedBundle

    & (Join-Path $extractedBundle 'run-phase3-benchmark.ps1') `
        -BinaryDirectory $extractedBundle -FilesPerProject 100 -Runs 1 -OutputPath $reportPath
    $report = Get-Content -LiteralPath $reportPath -Raw | ConvertFrom-Json
    if ($report.status -ne 'passed' -or $report.runs_requested -ne 1 -or
        $report.runs_completed -ne 1 -or $report.file_count_per_project -ne 100 -or
        @($report.samples).Count -ne 1) {
        throw 'Contained benchmark report did not pass the one-run fixture'
    }

    $tempAfter = @(Get-BenchmarkTempNames)
    if (@(Compare-Object -ReferenceObject $tempBefore -DifferenceObject $tempAfter |
        Where-Object { $_.SideIndicator -eq '=>' }).Count -ne 0) {
        throw 'Benchmark left an owned temporary project directory'
    }
    $daemonAfter = @(Get-Process -Name relayd -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
    if (@(Compare-Object -ReferenceObject $daemonBefore -DifferenceObject $daemonAfter |
        Where-Object { $_.SideIndicator -eq '=>' }).Count -ne 0) {
        throw 'Benchmark left a relayd process running'
    }

    Assert-NoPrivateIdentifiers $reportPath $true
    foreach ($file in Get-ChildItem -LiteralPath $extractedBundle -File -Recurse) {
        Assert-NoPrivateIdentifiers $file.FullName
    }
    if (-not (Test-Path -LiteralPath $reviewRoot)) {
        $null = New-Item -ItemType Directory -Path $reviewRoot
    }
    Copy-Item -LiteralPath $reportPath -Destination (Join-Path $reviewRoot 'hosted-runner-smoke.json')
    Write-Output 'Committed-source archive and contained 100-file hosted-runner smoke passed.'
}
finally {
    if (Test-Path -LiteralPath $reportPath -PathType Leaf) {
        Remove-Item -LiteralPath $reportPath -Force
    }
    if (Test-Path -LiteralPath $extractRoot) {
        $item = Get-Item -LiteralPath $extractRoot -Force
        $resolved = [System.IO.Path]::GetFullPath((Resolve-Path -LiteralPath $extractRoot).Path)
        if (-not $item.PSIsContainer -or ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -or
            [System.IO.Path]::GetDirectoryName($resolved) -ne $tempRoot -or
            [System.IO.Path]::GetFileName($resolved) -ne $extractName -or
            $extractName -notmatch '^relay-preview-ci-extract-[0-9a-f]{32}$') {
            throw 'Extraction cleanup path failed ownership validation'
        }
        $reparse = @(Get-ChildItem -LiteralPath $extractRoot -Recurse -Force |
            Where-Object { $_.Attributes -band [System.IO.FileAttributes]::ReparsePoint })
        if ($reparse.Count -ne 0) { throw 'Extracted archive contains a reparse point' }
        Remove-Item -LiteralPath $extractRoot -Recurse -Force
    }
}
