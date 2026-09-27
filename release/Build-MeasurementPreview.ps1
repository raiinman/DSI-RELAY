param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$')]
    [string]$Version,
    [string]$OutputDirectory,
    [string]$BinaryDirectory,
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$repoRoot = [System.IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
$previewTarget = Join-Path $repoRoot 'target\measurement-preview'
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $PSScriptRoot 'out' }
if (-not $BinaryDirectory) { $BinaryDirectory = Join-Path $previewTarget 'release' }
$outputRoot = [System.IO.Path]::GetFullPath($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory))
$binaryRoot = [System.IO.Path]::GetFullPath($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($BinaryDirectory))
$name = "relay-measurement-preview-$Version-windows-x64"
$finalDirectory = Join-Path $outputRoot $name
$finalArchive = Join-Path $outputRoot "$name.zip"
$finalArchiveHash = "$finalArchive.sha256"
$stageName = '.relay-preview-staging-' + [Guid]::NewGuid().ToString('N')
$stageRoot = Join-Path $outputRoot $stageName
$payloadRoot = Join-Path $stageRoot $name

function Get-Hash([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Assert-WindowsX64([string]$Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    try {
        $reader = New-Object System.IO.BinaryReader($stream)
        if ($reader.ReadUInt16() -ne 0x5a4d) { throw 'Binary is not a Windows PE executable' }
        $stream.Position = 0x3c
        $headerOffset = $reader.ReadInt32()
        if ($headerOffset -lt 0x40 -or $headerOffset -gt ($stream.Length - 6)) {
            throw 'Binary has an invalid Windows PE header'
        }
        $stream.Position = $headerOffset
        if ($reader.ReadUInt32() -ne 0x00004550 -or $reader.ReadUInt16() -ne 0x8664) {
            throw 'Binary is not a Windows x64 PE executable'
        }
    }
    finally { $stream.Dispose() }
}

function Assert-NoPersonalPath([string]$Path) {
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    $singleByte = [System.Text.Encoding]::GetEncoding(28591).GetString($bytes)
    $wide = [System.Text.Encoding]::Unicode.GetString($bytes)
    $needles = @($env:USERPROFILE, $env:USERNAME, 'C:\Users\', 'C:/Users/') |
        Where-Object { $_ -and $_.Length -ge 3 } | Select-Object -Unique
    foreach ($needle in $needles) {
        if ($singleByte.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -ge 0 -or
            $wide.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw 'Release binary contains a personal profile path or account identifier'
        }
    }
}

function Write-Utf8([string]$Path, [string]$Value) {
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, $Value.Replace("`r`n", "`n").Replace("`n", "`r`n"), $utf8)
}

function Get-NoticeFiles($Package) {
    $crateRoot = Split-Path ([string]$Package.manifest_path) -Parent
    $candidateFiles = @(Get-ChildItem -LiteralPath $crateRoot -File -ErrorAction Stop |
        Where-Object { $_.Name -match '^(?:LICENSE|LICENCE|COPYING|NOTICE|UNLICENSE|AUTHORS|COPYRIGHT)(?:[._-].*)?$' })
    if ($Package.license_file) {
        $declared = [System.IO.Path]::GetFullPath((Join-Path $crateRoot ([string]$Package.license_file)))
        if (-not $declared.StartsWith($crateRoot + [System.IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
            throw "Dependency license_file escapes its crate: $($Package.name)"
        }
        if (-not (Test-Path -LiteralPath $declared -PathType Leaf)) {
            throw "Dependency license_file is unavailable: $($Package.name)"
        }
        $candidateFiles += Get-Item -LiteralPath $declared
    }
    return @($candidateFiles | Sort-Object FullName -Unique)
}

function Get-RuntimePackages($Metadata) {
    $roots = @($Metadata.packages | Where-Object { $_.name -in @('relay', 'relayd') } | ForEach-Object { $_.id })
    if ($roots.Count -ne 2) { throw 'Could not identify both CLI and daemon packages in Cargo metadata' }
    $nodes = @{}
    foreach ($node in $Metadata.resolve.nodes) { $nodes[[string]$node.id] = $node }
    $seen = @{}
    $queue = New-Object 'System.Collections.Generic.Queue[string]'
    foreach ($id in $roots) { $queue.Enqueue([string]$id) }
    while ($queue.Count -gt 0) {
        $id = $queue.Dequeue()
        if ($seen.ContainsKey($id)) { continue }
        $seen[$id] = $true
        $node = $nodes[$id]
        if (-not $node) { throw "Missing dependency graph node: $id" }
        foreach ($dep in $node.deps) {
            if (@($dep.dep_kinds | Where-Object { $_.kind -ne 'dev' }).Count -gt 0) {
                $queue.Enqueue([string]$dep.pkg)
            }
        }
    }
    return @($Metadata.packages | Where-Object { $seen.ContainsKey([string]$_.id) -and $_.source -like 'registry+*' } |
        Sort-Object name, version)
}

function Get-SelectedLicense([string]$Expression) {
    switch ($Expression) {
        'MIT' { return 'MIT' }
        'MIT OR Apache-2.0' { return 'MIT' }
        'MIT/Apache-2.0' { return 'MIT' }
        'Unlicense OR MIT' { return 'MIT' }
        'Unlicense/MIT' { return 'MIT' }
        '(MIT OR Apache-2.0) AND Unicode-3.0' { return 'MIT AND Unicode-3.0' }
        'CC0-1.0' { return 'CC0-1.0' }
        default { return $null }
    }
}

function Test-SelectedLicenseFiles([string]$Selection, [string[]]$Names) {
    $available = @($Names | ForEach-Object { [System.IO.Path]::GetFileName($_) })
    if ($Selection -like 'MIT*' -and
        @($available | Where-Object { $_ -match '^(?i:LICENSE-MIT|LICENSE)$' }).Count -eq 0) { return $false }
    if ($Selection -eq 'MIT AND Unicode-3.0' -and
        @($available | Where-Object { $_ -match '^(?i:LICENSE-UNICODE)$' }).Count -eq 0) { return $false }
    if ($Selection -eq 'CC0-1.0' -and
        @($available | Where-Object { $_ -match '^(?i:LICENSE-CC0)$' }).Count -eq 0) { return $false }
    return $true
}

if (-not (Test-Path -LiteralPath $outputRoot)) {
    $null = New-Item -ItemType Directory -Path $outputRoot
}
$outputItem = Get-Item -LiteralPath $outputRoot -Force
if (-not $outputItem.PSIsContainer -or ($outputItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint)) {
    throw 'Output directory must be a normal directory, not a reparse point'
}
if ((Test-Path -LiteralPath $finalDirectory) -or (Test-Path -LiteralPath $finalArchive) -or
    (Test-Path -LiteralPath $finalArchiveHash)) {
    throw "Versioned preview artifact already exists: $name"
}

if (-not $SkipBuild) {
    if ($env:RUSTFLAGS) { throw 'Custom RUSTFLAGS must be cleared for the preview remapped build' }
    if ($env:CARGO_ENCODED_RUSTFLAGS) { throw 'Custom CARGO_ENCODED_RUSTFLAGS must be cleared for the preview remapped build' }
    if (-not $env:USERPROFILE) { throw 'USERPROFILE is required to remap local build paths' }
    $separator = [char]31
    $remapFlags = @(
        "--remap-path-prefix=$repoRoot=/_source",
        "--remap-path-prefix=$env:USERPROFILE=/_build"
    ) -join $separator
    $env:CARGO_ENCODED_RUSTFLAGS = $remapFlags
    Push-Location $repoRoot
    try {
        & cargo build --locked --release --target-dir $previewTarget -p relay -p relayd
        if ($LASTEXITCODE -ne 0) { throw 'Release binary build failed' }
    }
    finally {
        Pop-Location
        $env:CARGO_ENCODED_RUSTFLAGS = $null
    }
}

$metadata = $null
Push-Location $repoRoot
try {
    $metadataText = & cargo metadata --locked --offline --format-version 1 --filter-platform x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Offline dependency inventory failed' }
    $metadata = $metadataText | ConvertFrom-Json
}
finally { Pop-Location }

$lockHash = Get-Hash (Join-Path $repoRoot 'Cargo.lock')
$runtimePackages = @(Get-RuntimePackages $metadata)
$sourceRevision = (& git -C $repoRoot rev-parse HEAD | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $sourceRevision -notmatch '^[0-9a-f]{40}$') {
    throw 'Could not identify repository revision'
}
$workingTreeChanges = @(& git -C $repoRoot status --porcelain)
if ($LASTEXITCODE -ne 0) { throw 'Could not inspect repository worktree' }
$sourceBinaries = @{}
foreach ($binaryName in @('relay.exe', 'relayd.exe')) {
    $binaryPath = Join-Path $binaryRoot $binaryName
    if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) { throw "Missing release binary: $binaryName" }
    Assert-WindowsX64 $binaryPath
    Assert-NoPersonalPath $binaryPath
    $sourceBinaries[$binaryName] = Get-Hash $binaryPath
}
$stageFull = [System.IO.Path]::GetFullPath($stageRoot)
if ([System.IO.Path]::GetDirectoryName($stageFull) -ne $outputRoot -or
    [System.IO.Path]::GetFileName($stageFull) -ne $stageName -or
    $stageName -notmatch '^\.relay-preview-staging-[0-9a-f]{32}$') {
    throw 'Staging path failed validation'
}

try {
    $null = New-Item -ItemType Directory -Path $payloadRoot
    $runnerSource = Join-Path $repoRoot 'crates\relayd\tests\phase3_portable_benchmark.ps1'
    $verifierSource = Join-Path $PSScriptRoot 'Verify-MeasurementPreview.ps1'
    foreach ($pair in @(
        @{ Source = (Join-Path $binaryRoot 'relayd.exe'); Target = 'relayd.exe' },
        @{ Source = (Join-Path $binaryRoot 'relay.exe'); Target = 'relay.exe' },
        @{ Source = $runnerSource; Target = 'run-phase3-benchmark.ps1' },
        @{ Source = $verifierSource; Target = 'verify-bundle.ps1' }
    )) {
        if (-not (Test-Path -LiteralPath $pair.Source -PathType Leaf)) { throw "Missing payload: $($pair.Target)" }
        Copy-Item -LiteralPath $pair.Source -Destination (Join-Path $payloadRoot $pair.Target)
    }
    foreach ($binaryName in @('relay.exe', 'relayd.exe')) {
        if ((Get-Hash (Join-Path $binaryRoot $binaryName)) -ne $sourceBinaries[$binaryName] -or
            (Get-Hash (Join-Path $payloadRoot $binaryName)) -ne $sourceBinaries[$binaryName]) {
            throw "Release binary changed during assembly: $binaryName"
        }
    }

    $unresolved = New-Object 'System.Collections.Generic.List[string]'
    $licensePath = Join-Path $repoRoot 'LICENSE'
    if (Test-Path -LiteralPath $licensePath -PathType Leaf) {
        Copy-Item -LiteralPath $licensePath -Destination (Join-Path $payloadRoot 'LICENSE')
    }
    else { $unresolved.Add('RELAY_LICENSE_FILE_MISSING') }
    $attributionPath = Join-Path $repoRoot 'NOTICE-RELAY.txt'
    if (Test-Path -LiteralPath $attributionPath -PathType Leaf) {
        Copy-Item -LiteralPath $attributionPath -Destination (Join-Path $payloadRoot 'NOTICE-RELAY.txt')
    }
    else { $unresolved.Add('OWNER_ATTRIBUTION_PENDING') }

    $dependencyRows = @()
    $noticeLines = @(
        'RELAY third-party notice index',
        'Exact Windows CLI/daemon resolved graph; includes conservative build dependencies.',
        'For alternatives, the MIT option is selected where available. All original',
        'license texts collected from each crate are retained. This technical index',
        'does not grant legal release approval.',
        ''
    )
    $reviewedLockHash = '8f46c4a561e2716ffb43d0a9ade13979d9ee021147a6b3c7cb805854c5bc477c'
    $reviewedNoticeFingerprint = 'efbcc57bf0ec67fc9a91d682ac06080be04c6b0800c069a9ddaf2acb828532f5'
    $noticesValidated = ($lockHash -eq $reviewedLockHash -and $runtimePackages.Count -eq 42)
    $noticeFingerprintLines = @()
    foreach ($package in $runtimePackages) {
        $packageLabel = '{0}-{1}' -f $package.name, $package.version
        $noticeDirectory = Join-Path $payloadRoot ("third-party\$packageLabel")
        $texts = @(Get-NoticeFiles $package)
        if ($texts.Count -eq 0) { $unresolved.Add("NO_LICENSE_TEXT:$packageLabel") }
        $textEntries = @()
        foreach ($file in $texts) {
            if (-not (Test-Path -LiteralPath $noticeDirectory)) {
                $null = New-Item -ItemType Directory -Path $noticeDirectory
            }
            $relative = "third-party/$packageLabel/$($file.Name)"
            Copy-Item -LiteralPath $file.FullName -Destination (Join-Path $noticeDirectory $file.Name)
            $textEntries += $relative
        }
        $selection = Get-SelectedLicense ([string]$package.license)
        if (-not $selection -or -not (Test-SelectedLicenseFiles $selection $textEntries)) {
            $unresolved.Add("NOTICE_SELECTION_UNRESOLVED:$packageLabel")
            $noticesValidated = $false
        }
        $noticeLines += ('{0} | metadata: {1} | selected: {2}' -f $packageLabel, $package.license, $(if ($selection) { $selection } else { 'unresolved' }))
        $noticeLines += @($textEntries | ForEach-Object { "  $_" })
        foreach ($relative in $textEntries) {
            $digest = Get-Hash (Join-Path $payloadRoot $relative)
            $noticeFingerprintLines += ('{0}|{1}|{2}|{3}|{4}|{5}' -f
                $package.name, $package.version, $package.license, $selection, $relative, $digest)
        }
        $dependencyRows += [ordered]@{
            crate = [string]$package.name
            version = [string]$package.version
            license_expression = [string]$package.license
            selected_license = $selection
            license_files = @($textEntries)
        }
    }
    $fingerprintBytes = [System.Text.Encoding]::UTF8.GetBytes(($noticeFingerprintLines -join "`n"))
    $fingerprintHasher = [System.Security.Cryptography.SHA256]::Create()
    try {
        $noticeFingerprint = [BitConverter]::ToString($fingerprintHasher.ComputeHash($fingerprintBytes)).Replace('-', '').ToLowerInvariant()
    }
    finally { $fingerprintHasher.Dispose() }
    if ($noticeFingerprint -ne $reviewedNoticeFingerprint) { $noticesValidated = $false }
    if (-not $noticesValidated) { $unresolved.Add('THIRD_PARTY_NOTICE_SELECTION_REVIEW') }
    Write-Utf8 (Join-Path $payloadRoot 'THIRD-PARTY-NOTICES.txt') (($noticeLines -join "`n") + "`n")

    $sqliteValidated = $false
    $sqlitePackage = @($runtimePackages | Where-Object { $_.name -eq 'libsqlite3-sys' -and $_.version -eq '0.38.2' })
    if ($lockHash -eq $reviewedLockHash -and $sqlitePackage.Count -eq 1) {
        $sqliteNode = @($metadata.resolve.nodes | Where-Object { $_.id -eq $sqlitePackage[0].id })
        $sqliteRoot = Split-Path ([string]$sqlitePackage[0].manifest_path) -Parent
        $sqliteSource = Join-Path $sqliteRoot 'sqlite3\sqlite3.c'
        $sqliteHeader = Join-Path $sqliteRoot 'sqlite3\sqlite3.h'
        $expectedSourceId = '2026-06-03 19:12:13 d6e03d8c777cfa2d35e3b60d8ec3e0187f3e9f99d8e2ee9cac695fd6fcdf1a24'
        $expectedSourceSha256 = '0a409f1633283fa31a9126b11fbfd64a1991c5d30defad07e5745d4667f5e23d'
        $expectedHeaderSha256 = '9e69a1353a4288450b0d5239ede11fc7f1f4c8e5eb07491fc8317eacb5b7de7e'
        if ($sqliteNode.Count -eq 1 -and
            'bundled' -in $sqliteNode[0].features -and
            'bundled-sqlcipher' -notin $sqliteNode[0].features -and
            'sqlcipher' -notin $sqliteNode[0].features -and
            (Test-Path -LiteralPath $sqliteSource -PathType Leaf) -and
            (Test-Path -LiteralPath $sqliteHeader -PathType Leaf) -and
            (Get-Hash $sqliteSource) -eq $expectedSourceSha256 -and
            (Get-Hash $sqliteHeader) -eq $expectedHeaderSha256) {
            $headerText = Get-Content -LiteralPath $sqliteHeader -Raw
            if ($headerText.Contains('SQLITE_VERSION        "3.53.2"') -and
                $headerText.Contains($expectedSourceId)) { $sqliteValidated = $true }
        }
    }
    if ($sqliteValidated) {
        $sqliteNotice = @"
Bundled SQLite source provenance

The reviewed libsqlite3-sys 0.38.2 Windows graph enables bundled SQLite,
not SQLCipher. Its build script compiles sqlite3/sqlite3.c. The cached
amalgamation identifies SQLite 3.53.2 with SQLITE_SOURCE_ID:
$expectedSourceId

sqlite3.c SHA-256: $expectedSourceSha256
sqlite3.h SHA-256: $expectedHeaderSha256
SQLite's official 3.53.2 release history publishes this sqlite3.c SHA3-256:
44fd61b9f93b4155105cb2d80c957ae6c64a8b5bd6ed51a4992f0dbd438e4e11
The cached sqlite3.c produced that exact SHA3-256 in the release review.

SQLite states that its deliverable code is dedicated to the public domain:
https://www.sqlite.org/copyright.html
Official release history: https://www.sqlite.org/changes.html

This is a technical source match, not a legal opinion or release approval.
"@
        Write-Utf8 (Join-Path $payloadRoot 'SQLITE-PROVENANCE.txt') ($sqliteNotice + "`n")
    }
    else { $unresolved.Add('BUNDLED_SQLITE_PROVENANCE_REVIEW') }
    $unresolved.Add('LEGAL_RELEASE_APPROVAL_PENDING')
    $unresolved.Add('CLEAN_ACCOUNT_SMOKE_PENDING')
    $unresolved.Add('PUBLISHER_TRUST_AND_SECURITY_REVIEW')

    Write-Utf8 (Join-Path $payloadRoot 'THIRD-PARTY-INVENTORY.json') (($dependencyRows | ConvertTo-Json -Depth 6) + "`n")
    $readme = @'
RELAY MEASUREMENT PREVIEW - LOCAL REVIEW BUILD

This package measures generic file indexing on this Windows PC. It is a synthetic
benchmark, not an editor integration, installer, supported hardware promise, or
production-ready RELAY release. It does not use your existing projects.

Requirements: Windows PowerShell 5.1 or later. Run as a normal signed-in user.
Keep all files together. Open PowerShell in this folder, verify the files,
then run the benchmark:

  powershell -NoProfile -ExecutionPolicy Bypass -File .\verify-bundle.ps1

  powershell -NoProfile -ExecutionPolicy Bypass -File .\run-phase3-benchmark.ps1 -BinaryDirectory . -OutputPath ..\relay-benchmark.json

The default creates two temporary projects of 1,000 small text files each and
runs three samples. It may take several minutes and needs free disk space for
fixtures and daemon state. To try a quick smoke first, add:

  -FilesPerProject 100 -Runs 1

The runner starts a RELAY daemon for each sample, creates its own temporary
projects, measures indexing, watcher response, and idle resource use, then
shuts down that daemon and removes its verified temporary directory. If the
run fails, inspect the JSON status and failure code. A hard interruption can
leave temporary data or a daemon; see CLEAN-ACCOUNT-SMOKE.txt for checks.

PRIVACY: No report is uploaded automatically. The runner writes a JSON report
only at the OutputPath you choose. It does not read your projects. The report
includes CPU model/core counts, installed RAM, Windows version, storage and
power classes, binary hashes, counts, timings, memory, and correctness results.
It omits username, computer name, project paths/content, credentials, and the
local IPC token. Inspect it before choosing to share it with anyone.

KNOWN LIMITS: Synthetic small files on a local filesystem cannot prove editor
performance, real creator-app contention, parser performance, minimum or
recommended hardware support, or long-run power cost. The idle window is five
seconds per run. The package has no automatic updater or uninstall program.
Deleting this folder removes the package; reports you wrote outside remain.

This is a REVIEW-ONLY package. Check bundle-manifest.json for unresolved
licensing, trust, clean-account, and security items before public distribution.
'@
    Write-Utf8 (Join-Path $payloadRoot 'README.txt') ($readme + "`n")
    $smoke = @'
CLEAN WINDOWS ACCOUNT SMOKE CHECK

1. Use a separate standard Windows account with no RELAY state or source checkout.
2. Copy the entire versioned folder. Check SHA256SUMS.txt against local files.
3. Run the 100-file, one-sample command in README.txt. Then run the default.
4. Confirm both samples report success, two projects remain isolated, and the
   JSON contains no username, computer name, paths, source text, or IPC token.
5. Confirm no relayd.exe remains after each run and no relay-portable-benchmark-*
   directory from the run remains under that account's temporary directory.
6. Watch the network during the run and confirm the package needs no connection.
7. Run a handled failure (missing binary in a copy) and verify a local failed
   report with a fixed code and no raw daemon output.
8. Replace the versioned folder with a newer copy. Confirm older reports stay
   untouched and both copies can be removed without deleting user project data.

Record the Windows build, account type, storage class, package SHA-256, report
schema/version, pass/fail per step, and any residual process or directory.
This checklist is a test plan, not a completed clean-account result.
'@
    Write-Utf8 (Join-Path $payloadRoot 'CLEAN-ACCOUNT-SMOKE.txt') ($smoke + "`n")

    $files = @(Get-ChildItem -LiteralPath $payloadRoot -File -Recurse | ForEach-Object {
        $relative = $_.FullName.Substring($payloadRoot.Length + 1).Replace('\', '/')
        [ordered]@{ path = $relative; bytes = [int64]$_.Length; sha256 = Get-Hash $_.FullName }
    } | Sort-Object path)
    $manifest = [ordered]@{
        schema_version = 1
        bundle_name = $name
        bundle_version = $Version
        platform = 'windows-x64'
        cargo_lock_sha256 = $lockHash
        source_revision = $sourceRevision
        source_tree_clean = ($workingTreeChanges.Count -eq 0)
        status = 'review_only'
        binary_build_mode = $(if ($SkipBuild) { 'prebuilt_supplied' } else { 'built_by_packager' })
        source_binary_provenance = 'Binary-to-revision correspondence is not independently attested'
        third_party_crate_count = $runtimePackages.Count
        third_party_notice_fingerprint_sha256 = $noticeFingerprint
        third_party_notice_review = $(if ($noticesValidated) { 'technical_selection_recorded' } else { 'review_required' })
        bundled_sqlite_provenance = $(if ($sqliteValidated) { 'source_hash_and_features_verified' } else { 'review_required' })
        unresolved_checks = @($unresolved | Sort-Object -Unique)
        files = $files
    }
    Write-Utf8 (Join-Path $payloadRoot 'bundle-manifest.json') (($manifest | ConvertTo-Json -Depth 8) + "`n")
    $hashRows = @(Get-ChildItem -LiteralPath $payloadRoot -File -Recurse | ForEach-Object {
        $relative = $_.FullName.Substring($payloadRoot.Length + 1).Replace('\', '/')
        '{0}  {1}' -f (Get-Hash $_.FullName), $relative
    } | Sort-Object)
    Write-Utf8 (Join-Path $payloadRoot 'SHA256SUMS.txt') (($hashRows -join "`n") + "`n")

    Add-Type -AssemblyName System.IO.Compression
    $zipPath = Join-Path $stageRoot "$name.zip"
    $stream = [System.IO.File]::Open($zipPath, [System.IO.FileMode]::CreateNew)
    try {
        $zip = New-Object System.IO.Compression.ZipArchive($stream, [System.IO.Compression.ZipArchiveMode]::Create, $false)
        try {
            foreach ($file in @(Get-ChildItem -LiteralPath $payloadRoot -File -Recurse | Sort-Object FullName)) {
                $relative = $file.FullName.Substring($payloadRoot.Length + 1).Replace('\', '/')
                $entry = $zip.CreateEntry("$name/$relative", [System.IO.Compression.CompressionLevel]::Optimal)
                $entry.LastWriteTime = [DateTimeOffset]::Parse('2000-01-01T00:00:00+00:00')
                $entryStream = $entry.Open()
                try {
                    $inputStream = [System.IO.File]::OpenRead($file.FullName)
                    try { $inputStream.CopyTo($entryStream) }
                    finally { $inputStream.Dispose() }
                }
                finally { $entryStream.Dispose() }
            }
        }
        finally { $zip.Dispose() }
    }
    finally { $stream.Dispose() }

    $archiveHash = Get-Hash $zipPath
    Move-Item -LiteralPath $payloadRoot -Destination $finalDirectory
    Move-Item -LiteralPath $zipPath -Destination $finalArchive
    Write-Utf8 $finalArchiveHash ("$archiveHash  $name.zip`n")
    Write-Output "Review bundle: $finalDirectory"
    Write-Output "Archive: $finalArchive"
    Write-Output "Archive SHA-256: $archiveHash"
    Write-Output "Unresolved checks: $($manifest.unresolved_checks -join ', ')"
}
finally {
    if (Test-Path -LiteralPath $stageRoot) {
        $stageItem = Get-Item -LiteralPath $stageRoot -Force
        $stageResolved = (Resolve-Path -LiteralPath $stageRoot).Path
        if (-not $stageItem.PSIsContainer -or ($stageItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -or
            [System.IO.Path]::GetDirectoryName($stageResolved) -ne $outputRoot -or
            [System.IO.Path]::GetFileName($stageResolved) -ne $stageName) {
            throw 'Staging cleanup path failed validation'
        }
        Remove-Item -LiteralPath $stageRoot -Recurse -Force
    }
}
