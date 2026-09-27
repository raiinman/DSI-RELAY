param(
    [ValidateSet('Verify', 'Reference', 'Describe', 'Invoke')]
    [Parameter(Mandatory = $true)][string]$Action,
    [string]$RelayPath = 'relay',
    [string]$Prefix,
    [ValidateRange(1, 20)][int]$Limit = 8,
    [string]$CommandId,
    [string]$ArgumentsFile,
    [string]$IdempotencyKey,
    [switch]$AllowStateWrite
)

$ErrorActionPreference = 'Stop'
$referencePath = Join-Path $PSScriptRoot '..\references\commands.generated.json'
$reference = Get-Content -LiteralPath $referencePath -Raw | ConvertFrom-Json

function Invoke-Relay([string]$Id, [object]$Arguments, [string]$Key = '', [int]$Version = 1) {
    $request = [ordered]@{ command = $Id; command_version = $Version; arguments = $Arguments }
    if ($Key) { $request.idempotency_key = $Key }
    $inputJson = $request | ConvertTo-Json -Depth 64 -Compress
    $responseText = $inputJson | & $RelayPath exec --stdin --json
    $relayExit = $LASTEXITCODE
    if (!$responseText) { throw "RELAY returned no structured response (exit $relayExit)" }
    $response = ($responseText -join "`n") | ConvertFrom-Json
    return [pscustomobject]@{ Body = $response; Exit = $relayExit }
}

function Test-Compatibility([string]$NeededCapability = '') {
    $sourceRegistry = Join-Path $PSScriptRoot '..\..\..\crates\relay-contracts\commands.registry.json'
    if (Test-Path -LiteralPath $sourceRegistry) {
        $sourceHash = (Get-FileHash -LiteralPath $sourceRegistry -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($sourceHash -ne $reference.registry_sha256) {
            throw 'Generated command reference is stale; run generate-reference.ps1'
        }
    }
    $reply = Invoke-Relay 'system.status' ([pscustomobject]@{})
    if ($reply.Exit -ne 0 -or !$reply.Body.ok) { throw 'RELAY host status is unavailable' }
    $status = $reply.Body.result
    if (!$status.version.StartsWith("$($reference.relay_version_family).")) {
        throw "RELAY version $($status.version) is outside skill family $($reference.relay_version_family).x"
    }
    foreach ($capability in $reference.required_capabilities) {
        if ($status.capabilities -notcontains $capability) {
            throw "RELAY host lacks required capability $capability"
        }
    }
    if ($NeededCapability -and $status.capabilities -notcontains $NeededCapability) {
        throw "RELAY host lacks command capability $NeededCapability"
    }
    [pscustomobject]@{
        compatible = $true
        relay_version = $status.version
        skill_generation_version = $reference.generation_version
        registry_sha256 = $reference.registry_sha256
    }
}

switch ($Action) {
    'Verify' {
        Test-Compatibility | ConvertTo-Json -Compress
    }
    'Reference' {
        if (!$Prefix) { throw 'Reference requires -Prefix to avoid a full catalog dump' }
        @($reference.commands | Where-Object { $_.id.StartsWith($Prefix) } | Select-Object -First $Limit) |
            ConvertTo-Json -Depth 8
    }
    'Describe' {
        if (!$CommandId) { throw 'Describe requires -CommandId' }
        $reply = Invoke-Relay 'registry.describe' ([pscustomobject]@{ command = $CommandId })
        $reply.Body | ConvertTo-Json -Depth 64 -Compress
        exit $reply.Exit
    }
    'Invoke' {
        if (!$CommandId -or !$ArgumentsFile) { throw 'Invoke requires -CommandId and -ArgumentsFile' }
        $metadata = @($reference.commands | Where-Object id -eq $CommandId | Sort-Object version -Descending | Select-Object -First 1)
        if ($metadata.Count -ne 1) { throw "Command $CommandId is not in the generated AI reference; describe it live and regenerate this skill" }
        if ($metadata[0].effect_class -notin @('observe', 'analyze') -and !$AllowStateWrite) {
            throw "Command $CommandId changes state; pass -AllowStateWrite after reviewing its live contract"
        }
        if ($metadata[0].idempotency -eq 'keyed' -and !$IdempotencyKey) {
            throw "Command $CommandId requires -IdempotencyKey"
        }
        $null = Test-Compatibility "$CommandId@$($metadata[0].version)"
        $arguments = Get-Content -LiteralPath $ArgumentsFile -Raw | ConvertFrom-Json
        if ($arguments -isnot [pscustomobject]) { throw 'ArgumentsFile must contain one JSON object' }
        $reply = Invoke-Relay $CommandId $arguments $IdempotencyKey $metadata[0].version
        $reply.Body | ConvertTo-Json -Depth 64 -Compress
        exit $reply.Exit
    }
}
