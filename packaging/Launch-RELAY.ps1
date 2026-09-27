$ErrorActionPreference = 'Stop'

try {
    $relayPath = Join-Path $PSScriptRoot 'relay.exe'
    if (-not (Test-Path -LiteralPath $relayPath -PathType Leaf)) {
        throw 'The RELAY program file is missing.'
    }
    & $relayPath launch
    if ($LASTEXITCODE -ne 0) {
        throw 'RELAY could not start. Open RELAY from a terminal for more details.'
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
