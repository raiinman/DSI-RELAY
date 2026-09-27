$ErrorActionPreference = 'Stop'

try {
    $relayPath = Join-Path $PSScriptRoot 'relay.exe'
    if (-not (Test-Path -LiteralPath $relayPath -PathType Leaf)) {
        throw 'The RELAY program file is missing.'
    }
    $launchOutput = & $relayPath launch 2>&1
    if ($LASTEXITCODE -ne 0) {
        $detail = (($launchOutput | ForEach-Object { [string]$_ }) -join ' ').Trim()
        $detail = ($detail -replace '[\x00-\x1F]', ' ').Trim()
        if ($detail.Length -gt 400) { $detail = $detail.Substring(0, 400) }
        if (-not $detail) { $detail = 'Run relay.exe launch from the RELAY installation folder for details.' }
        throw "RELAY could not start: $detail"
    }
}
catch {
    Add-Type -AssemblyName System.Windows.Forms
    [void][System.Windows.Forms.MessageBox]::Show(
        $_.Exception.Message,
        'DSI RELAY',
        [System.Windows.Forms.MessageBoxButtons]::OK,
        [System.Windows.Forms.MessageBoxIcon]::Error
    )
    exit 1
}
