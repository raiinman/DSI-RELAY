param(
    [string]$RegistryPath = (Join-Path $PSScriptRoot '..\..\..\crates\relay-contracts\commands.registry.json'),
    [string]$OutputPath = (Join-Path $PSScriptRoot '..\references\commands.generated.json')
)

$ErrorActionPreference = 'Stop'
$registryFile = (Resolve-Path -LiteralPath $RegistryPath).Path
$registry = Get-Content -LiteralPath $registryFile -Raw | ConvertFrom-Json
if ($registry.registry_format -ne 1 -or $registry.registry_id -ne 'dsi.relay.commands') {
    throw 'Unsupported RELAY command registry'
}
$commands = @($registry.commands |
    Where-Object { $_.surfaces -contains 'ai' } |
    Sort-Object id, version |
    ForEach-Object {
        [ordered]@{
            id = $_.id
            version = [int]$_.version
            summary = $_.summary
            effect_class = $_.effect_class
            permission = $_.permission
            idempotency = $_.idempotency
        }
    })
$reference = [ordered]@{
    generation_version = 1
    relay_version_family = '0.1'
    registry_format = [int]$registry.registry_format
    registry_id = $registry.registry_id
    registry_sha256 = (Get-FileHash -LiteralPath $registryFile -Algorithm SHA256).Hash.ToLowerInvariant()
    required_capabilities = @('system.status@1', 'system.doctor@1', 'registry.list@1', 'registry.describe@1', 'result.describe@1', 'result.context@1')
    commands = $commands
}
$destination = [System.IO.Path]::GetFullPath($OutputPath)
[System.IO.Directory]::CreateDirectory([System.IO.Path]::GetDirectoryName($destination)) | Out-Null
$json = $reference | ConvertTo-Json -Depth 8
[System.IO.File]::WriteAllText($destination, $json + "`n", [System.Text.UTF8Encoding]::new($false))
