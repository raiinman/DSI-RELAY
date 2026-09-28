$ErrorActionPreference = 'Stop'

try {
    $relayPath = Join-Path $PSScriptRoot 'relay.exe'
    if (-not (Test-Path -LiteralPath $relayPath -PathType Leaf)) {
        throw 'The RELAY program file is missing.'
    }
    # Do not capture the launcher's output in a PowerShell pipeline. The
    # background engine can inherit that pipe and keep this launcher alive.
    & $relayPath launch
    if ($LASTEXITCODE -ne 0) {
        throw 'RELAY could not start. Run relay.exe launch from the RELAY installation folder for details.'
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
