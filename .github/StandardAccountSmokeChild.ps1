param(
    [Parameter(Mandatory = $true)][string]$BundleDirectory,
    [Parameter(Mandatory = $true)][string]$WorkDirectory,
    [Parameter(Mandatory = $true)][string]$ExpectedSid
)

$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = New-Object Security.Principal.WindowsPrincipal($identity)
$process = Get-Process -Id $PID
$check = [ordered]@{
    schema_version = 1
    identity_matches_temporary_account = ($identity.User.Value -eq $ExpectedSid)
    administrator_token = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    nonzero_windows_session = ($process.SessionId -gt 0)
    profile_loaded = (Test-Path -LiteralPath ('Registry::HKEY_USERS\' + $ExpectedSid + '\Software') -PathType Container)
}
$checkPath = Join-Path $WorkDirectory 'account-check.json'
$utf8 = New-Object System.Text.UTF8Encoding($false)
[IO.File]::WriteAllText($checkPath, (($check | ConvertTo-Json -Depth 4) + [Environment]::NewLine), $utf8)
if (-not $check.identity_matches_temporary_account -or $check.administrator_token -or
    -not $check.nonzero_windows_session -or -not $check.profile_loaded) {
    exit 2
}

# The runner's inherited temporary and app-data paths may refer to its admin
# account. Keep all benchmark state within this new account's owned directory.
foreach ($key in @('TEMP', 'TMP', 'LOCALAPPDATA', 'APPDATA')) {
    $path = Join-Path $WorkDirectory $key.ToLowerInvariant()
    $null = New-Item -ItemType Directory -Path $path -Force
    [Environment]::SetEnvironmentVariable($key, $path, 'Process')
}
Remove-Item Env:RELAY_STATE_DIR -ErrorAction SilentlyContinue
Remove-Item Env:RELAY_INSTANCE -ErrorAction SilentlyContinue

$verifier = Join-Path $BundleDirectory 'verify-bundle.ps1'
& $verifier
if (-not $?) { exit 3 }

$reportPath = Join-Path $WorkDirectory 'standard-account-benchmark.json'
& (Join-Path $BundleDirectory 'run-phase3-benchmark.ps1') `
    -BinaryDirectory $BundleDirectory -FilesPerProject 100 -Runs 1 -OutputPath $reportPath
if (-not $?) { exit 4 }
