param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$BundleOutputDirectory,
    [Parameter(Mandatory = $true)][string]$ReviewDirectory
)

$ErrorActionPreference = 'Stop'
$repoRoot = [IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
$programData = [IO.Path]::GetFullPath($env:ProgramData).TrimEnd('\')
$id = [Guid]::NewGuid().ToString('N')
$accountName = 'relayci' + $id.Substring(0, 12)
$workName = 'relay-standard-smoke-' + $id
$workRoot = Join-Path $programData $workName
$bundleName = "relay-measurement-preview-$Version-windows-x64"
$bundle = Join-Path ([IO.Path]::GetFullPath($BundleOutputDirectory)) $bundleName
$reviewRoot = [IO.Path]::GetFullPath($ReviewDirectory)
$createdAccount = $false
$child = $null
$failure = $null
$stage = 'preflight'
$daemonBefore = @(Get-Process -Name relayd -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)

function Assert-PrivateDataAbsent([string]$Path, [string[]]$Needles) {
    $raw = [IO.File]::ReadAllText($Path)
    foreach ($needle in $Needles) {
        if ($needle -and $needle.Length -ge 4 -and
            $raw.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw 'Account report contains a host or account identifier'
        }
    }
    if ($raw -match '(?i)C:[\\/]Users[\\/]|(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})') {
        throw 'Account report contains a user path or token-shaped value'
    }
}

function Remove-OwnedWorkRoot {
    if (-not (Test-Path -LiteralPath $workRoot)) { return }
    $resolved = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $workRoot).Path)
    $item = Get-Item -LiteralPath $workRoot -Force
    if (-not $item.PSIsContainer -or
        ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
        [IO.Path]::GetDirectoryName($resolved) -ne $programData -or
        [IO.Path]::GetFileName($resolved) -ne $workName -or
        $workName -notmatch '^relay-standard-smoke-[0-9a-f]{32}$') {
        throw 'Standard-account cleanup path failed ownership validation'
    }
    $pending = New-Object 'System.Collections.Generic.Stack[string]'
    $pending.Push($resolved)
    while ($pending.Count -gt 0) {
        $current = $pending.Pop()
        foreach ($entry in Get-ChildItem -LiteralPath $current -Force) {
            if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw 'Standard-account work directory contains a reparse point'
            }
            if ($entry.PSIsContainer) { $pending.Push($entry.FullName) }
        }
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}

try {
    if (-not (Test-Path -LiteralPath $bundle -PathType Container)) {
        throw 'Verified preview bundle is missing'
    }
    $null = New-Item -ItemType Directory -Path $workRoot

    $stage = 'account_creation'
    $random = New-Object byte[] 48
    $rng = [Security.Cryptography.RandomNumberGenerator]::Create()
    try { $rng.GetBytes($random) } finally { $rng.Dispose() }
    $password = [Convert]::ToBase64String($random)
    [Array]::Clear($random, 0, $random.Length)
    $secure = ConvertTo-SecureString $password -AsPlainText -Force
    $account = New-LocalUser -Name $accountName -Password $secure -Description 'Ephemeral RELAY CI standard-account smoke'
    $createdAccount = $true
    $sid = $account.SID.Value
    $credential = New-Object Management.Automation.PSCredential("$env:COMPUTERNAME\$accountName", $secure)

    $stage = 'staging'
    $acl = Get-Acl -LiteralPath $workRoot
    $sidObject = New-Object Security.Principal.SecurityIdentifier($sid)
    $rights = [Security.AccessControl.FileSystemRights]::Modify
    $inheritance = [Security.AccessControl.InheritanceFlags]'ContainerInherit, ObjectInherit'
    $rule = New-Object Security.AccessControl.FileSystemAccessRule(
        $sidObject, $rights, $inheritance,
        [Security.AccessControl.PropagationFlags]::None,
        [Security.AccessControl.AccessControlType]::Allow)
    $acl.AddAccessRule($rule)
    Set-Acl -LiteralPath $workRoot -AclObject $acl

    $stagedBundle = Join-Path $workRoot $bundleName
    Copy-Item -LiteralPath $bundle -Destination $stagedBundle -Recurse
    $childScript = Join-Path $workRoot 'standard-account-smoke.ps1'
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'StandardAccountSmokeChild.ps1') -Destination $childScript
    $stdout = Join-Path $workRoot 'child-stdout.txt'
    $stderr = Join-Path $workRoot 'child-stderr.txt'

    $stage = 'secondary_logon'
    $args = '-NoProfile -ExecutionPolicy Bypass -File "{0}" -BundleDirectory "{1}" -WorkDirectory "{2}" -ExpectedSid "{3}"' -f `
        $childScript, $stagedBundle, $workRoot, $sid
    $child = Start-Process -FilePath (Join-Path $PSHOME 'powershell.exe') `
        -ArgumentList $args -Credential $credential -LoadUserProfile -PassThru `
        -WindowStyle Hidden -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    if (-not $child.WaitForExit(600000)) { throw 'Secondary-logon process timed out' }
    # Windows PowerShell 5.1 can expose a null ExitCode after redirected
    # Start-Process. The required child-produced JSON is authoritative.
    if ($null -ne $child.ExitCode -and $child.ExitCode -ne 0) {
        throw 'Secondary-logon process failed'
    }

    $stage = 'report_validation'
    $checkPath = Join-Path $workRoot 'account-check.json'
    $reportPath = Join-Path $workRoot 'standard-account-benchmark.json'
    $check = Get-Content -LiteralPath $checkPath -Raw | ConvertFrom-Json
    $report = Get-Content -LiteralPath $reportPath -Raw | ConvertFrom-Json
    if (-not $check.identity_matches_temporary_account -or $check.administrator_token -or
        -not $check.nonzero_windows_session -or -not $check.profile_loaded -or
        $report.status -ne 'passed' -or $report.runs_requested -ne 1 -or
        $report.runs_completed -ne 1 -or $report.file_count_per_project -ne 100 -or
        @($report.samples).Count -ne 1) {
        throw 'Standard-account evidence is incomplete'
    }
    foreach ($field in @('both_baselines_complete', 'attachment_verified_content',
        'clean_metadata_hashed_zero', 'watcher_observed_change',
        'other_project_remained_ready', 'post_hint_hashed_zero',
        'explicit_full_verify_complete')) {
        if ($report.samples[0].correctness.$field -ne $true) {
            throw 'Standard-account correctness check failed'
        }
    }
    if (@(Get-ChildItem -LiteralPath (Join-Path $workRoot 'temp') -Directory -Filter 'relay-portable-benchmark-*').Count -ne 0) {
        throw 'Standard-account benchmark left a temporary fixture'
    }
    $daemonAfter = @(Get-Process -Name relayd -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
    if (@(Compare-Object -ReferenceObject $daemonBefore -DifferenceObject $daemonAfter |
        Where-Object { $_.SideIndicator -eq '=>' }).Count -ne 0) {
        throw 'Standard-account benchmark left a daemon'
    }
    $needles = @($env:USERNAME, $env:COMPUTERNAME, $env:USERPROFILE, $accountName, $workRoot, $password)
    Assert-PrivateDataAbsent $checkPath $needles
    Assert-PrivateDataAbsent $reportPath $needles
    if (-not (Test-Path -LiteralPath $reviewRoot)) {
        $null = New-Item -ItemType Directory -Path $reviewRoot
    }
    Copy-Item -LiteralPath $checkPath -Destination (Join-Path $reviewRoot 'hosted-standard-account-check.json')
    Copy-Item -LiteralPath $reportPath -Destination (Join-Path $reviewRoot 'hosted-standard-account-smoke.json')
    Write-Output 'Temporary standard-account smoke passed; interactive desktop sign-in remains untested.'
}
catch {
    $failure = 'Hosted standard-account smoke failed at ' + $stage
}
finally {
    if ($child -and -not $child.HasExited) {
        try { Stop-Process -Id $child.Id -Force -ErrorAction Stop } catch { $failure = 'Standard-account child cleanup failed' }
    }
    $daemonAfter = @(Get-Process -Name relayd -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
    foreach ($pidToStop in @(Compare-Object -ReferenceObject $daemonBefore -DifferenceObject $daemonAfter |
        Where-Object { $_.SideIndicator -eq '=>' } | Select-Object -ExpandProperty InputObject)) {
        try { Stop-Process -Id $pidToStop -Force -ErrorAction Stop } catch { $failure = 'Standard-account daemon cleanup failed' }
    }
    try { Remove-OwnedWorkRoot } catch { $failure = 'Standard-account work directory cleanup failed' }
    if ($createdAccount) {
        try { Remove-LocalUser -Name $accountName -ErrorAction Stop }
        catch { $failure = 'Standard-account removal failed' }
    }
}
if ($failure) { throw $failure }
