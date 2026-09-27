$ErrorActionPreference = 'Stop'
$programRoot = Join-Path $env:LOCALAPPDATA 'Programs\DSI-RELAY'
$menuRoot = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
$shortcutPath = Join-Path $menuRoot 'DSI RELAY.lnk'
$pointerPath = Join-Path $programRoot 'current.json'
if (Test-Path -LiteralPath $pointerPath) {
    $pointerItem = Get-Item -LiteralPath $pointerPath -Force
    if ($pointerItem.PSIsContainer -or ($pointerItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Installed RELAY version pointer is redirected.'
    }
    $pointer = Get-Content -LiteralPath $pointerPath -Raw | ConvertFrom-Json
    if ($pointer.schema_version -ne 1 -or $pointer.channel -ne 'unsigned_local_development' -or
        [string]$pointer.active_version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$' -or
        [string]$pointer.archive_sha256 -notmatch '^[0-9a-f]{64}$') {
        throw 'Installed RELAY version pointer is invalid.'
    }
    $versionPath = Join-Path (Join-Path $programRoot 'versions') ([string]$pointer.active_version)
    $receiptPath = Join-Path (Join-Path $programRoot 'receipts') ("$($pointer.active_version).json")
    $receipt = Get-Content -LiteralPath $receiptPath -Raw -ErrorAction Stop | ConvertFrom-Json
    if ($receipt.version -ne $pointer.active_version -or
        $receipt.archive_sha256 -ne $pointer.archive_sha256) {
        throw 'Installed RELAY receipt does not match the active version.'
    }
    & (Join-Path $PSScriptRoot 'Verify-UnsignedStage.ps1') -PackageDirectory $versionPath
    $relayPath = Join-Path $versionPath 'relay.exe'
    $daemonPath = Join-Path $versionPath 'relayd.exe'
    $hostPath = Join-Path (Join-Path $env:LOCALAPPDATA 'DSI\RELAY') 'host.json'
    if (Test-Path -LiteralPath $hostPath) {
        $hostItem = Get-Item -LiteralPath $hostPath -Force
        if ($hostItem.PSIsContainer -or ($hostItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw 'RELAY engine state is redirected.'
        }
        $hostState = Get-Content -LiteralPath $hostPath -Raw | ConvertFrom-Json
        if ([string]$hostState.pid -match '^[1-9][0-9]*$') {
            $daemon = Get-Process -Id ([int]$hostState.pid) -ErrorAction SilentlyContinue
            if ($daemon -and [string]::Equals($daemon.Path, $daemonPath, [StringComparison]::OrdinalIgnoreCase)) {
                & $relayPath shutdown
                if ($LASTEXITCODE -ne 0) { throw 'RELAY could not stop its engine before removal.' }
                for ($attempt = 0; $attempt -lt 50; $attempt++) {
                    if (-not (Get-Process -Id $daemon.Id -ErrorAction SilentlyContinue)) { break }
                    Start-Sleep -Milliseconds 100
                }
                if (Get-Process -Id $daemon.Id -ErrorAction SilentlyContinue) {
                    throw 'RELAY engine did not stop before removal.'
                }
            }
        }
    }
}
if (Test-Path -LiteralPath $shortcutPath) {
    $item = Get-Item -LiteralPath $shortcutPath -Force
    if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'RELAY shortcut is redirected.'
    }
    $shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut($shortcutPath)
    $expectedPrefix = Join-Path (Join-Path $programRoot 'versions') ''
    if ($shortcut.Description -ne 'Launch DSI RELAY' -or
        -not $shortcut.Arguments.Contains($expectedPrefix) -or
        -not $shortcut.Arguments.Contains('Launch-RELAY.ps1')) {
        throw 'Start menu item named DSI RELAY is not owned by this installation.'
    }
}
& (Join-Path $PSScriptRoot 'Uninstall-LocalStage.ps1') -AllVersions -AllowUnsignedLocalDevelopment
if (Test-Path -LiteralPath $shortcutPath) {
    Remove-Item -LiteralPath $shortcutPath -Force
}
Write-Output 'DSI RELAY program removed. Your project files and RELAY data were kept.'
