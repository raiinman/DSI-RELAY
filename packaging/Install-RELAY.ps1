$ErrorActionPreference = 'Stop'

$archives = @(Get-ChildItem -LiteralPath $PSScriptRoot -File | Where-Object {
    $_.Name -match '^relay-[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?-windows-x64-unsigned-stage\.zip$'
})
if ($archives.Count -ne 1) { throw 'Expected exactly one RELAY package beside this installer.' }
$archive = $archives[0]
$hashPath = "$($archive.FullName).sha256"
$hashFile = Get-Item -LiteralPath $hashPath -Force -ErrorAction Stop
if ($hashFile.PSIsContainer -or ($hashFile.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'Package digest must be a regular file.'
}
$digestLine = (Get-Content -LiteralPath $hashPath -Raw).Trim()
if ($digestLine -notmatch '^([0-9a-f]{64})  ([A-Za-z0-9._-]+\.zip)$' -or
    $Matches[2] -cne $archive.Name) {
    throw 'Package digest file is invalid.'
}
$expectedDigest = $Matches[1]

& (Join-Path $PSScriptRoot 'Install-LocalStage.ps1') `
    -ArchivePath $archive.FullName `
    -ExpectedArchiveSha256 $expectedDigest `
    -AllowUnsignedLocalDevelopment -Activate

$programRoot = Join-Path $env:LOCALAPPDATA 'Programs\DSI-RELAY'
$pointerPath = Join-Path $programRoot 'current.json'
$pointer = Get-Content -LiteralPath $pointerPath -Raw -ErrorAction Stop | ConvertFrom-Json
if ($pointer.archive_sha256 -cne $expectedDigest -or
    [string]$pointer.active_version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$') {
    throw 'Installed RELAY version does not match this package.'
}
$versionPath = Join-Path (Join-Path $programRoot 'versions') ([string]$pointer.active_version)
$launchScript = Join-Path $versionPath 'Launch-RELAY.ps1'
if (-not (Test-Path -LiteralPath $launchScript -PathType Leaf)) {
    throw 'Installed RELAY launcher is missing.'
}
$powershellPath = Join-Path $PSHOME 'powershell.exe'
if (-not (Test-Path -LiteralPath $powershellPath -PathType Leaf)) {
    $powershellPath = Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe'
}
$menuRoot = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
if (-not (Test-Path -LiteralPath $menuRoot -PathType Container)) {
    throw 'Windows Start menu folder is unavailable.'
}
$shortcutPath = Join-Path $menuRoot 'DSI RELAY.lnk'
if (Test-Path -LiteralPath $shortcutPath) {
    $shortcutItem = Get-Item -LiteralPath $shortcutPath -Force
    if ($shortcutItem.PSIsContainer -or ($shortcutItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Existing RELAY shortcut is redirected.'
    }
    $existing = (New-Object -ComObject WScript.Shell).CreateShortcut($shortcutPath)
    $expectedProgramPrefix = Join-Path (Join-Path $programRoot 'versions') ''
    if ($existing.Description -ne 'Launch DSI RELAY' -or
        -not $existing.Arguments.Contains($expectedProgramPrefix) -or
        -not $existing.Arguments.Contains('Launch-RELAY.ps1')) {
        throw 'A Start menu item named DSI RELAY already belongs to another program.'
    }
}
$shortcut = (New-Object -ComObject WScript.Shell).CreateShortcut($shortcutPath)
$shortcut.TargetPath = $powershellPath
$shortcut.Arguments = '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "' + $launchScript + '"'
$shortcut.WorkingDirectory = $versionPath
$shortcut.Description = 'Launch DSI RELAY'
$shortcut.WindowStyle = 7
$shortcut.Save()

Write-Output 'DSI RELAY is installed. Open DSI RELAY from the Start menu to launch it.'
Write-Output 'This is an unsigned local test package; Windows may display a warning.'
