$ErrorActionPreference = 'Stop'

try {
    if (-not $env:LOCALAPPDATA) { throw 'LOCALAPPDATA is unavailable.' }
    $programRoot = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'Programs\DSI-RELAY'))
    $pointerPath = Join-Path $programRoot 'current.json'
    $pointerItem = Get-Item -LiteralPath $pointerPath -Force -ErrorAction Stop
    if ($pointerItem.PSIsContainer -or ($pointerItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'RELAY install pointer is not a regular file.'
    }
    $pointer = Get-Content -LiteralPath $pointerPath -Raw -ErrorAction Stop | ConvertFrom-Json
    $version = [string]$pointer.active_version
    if ($version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$') {
        throw 'RELAY install pointer has an invalid version.'
    }
    $versionsRoot = Join-Path $programRoot 'versions'
    $versionPath = Join-Path $versionsRoot $version
    $versionItem = Get-Item -LiteralPath $versionPath -Force -ErrorAction Stop
    if (-not $versionItem.PSIsContainer -or ($versionItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'RELAY installed version is not a regular directory.'
    }
    $gatewayPath = Join-Path $versionPath 'relay-gateway.exe'
    $gatewayItem = Get-Item -LiteralPath $gatewayPath -Force -ErrorAction Stop
    if ($gatewayItem.PSIsContainer -or ($gatewayItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'RELAY gateway is not a regular executable.'
    }
    & $gatewayPath stdio
    exit $LASTEXITCODE
}
catch {
    [Console]::Error.WriteLine('DSI RELAY Chat adapter could not start. Install or open DSI RELAY, then retry the Chat connection. ' + $_.Exception.Message)
    exit 1
}
