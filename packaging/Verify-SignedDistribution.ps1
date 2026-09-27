#requires -Version 7.2

param(
    [string]$PayloadDirectory,
    [string]$CatalogPath,
    [string]$SignToolPath,
    [string]$ExpectedCatalogSha256,
    [string]$ExpectedPublisherSubject,
    [string]$ExpectedSignerThumbprint,
    [string]$ExpectedRootThumbprint
)

$ErrorActionPreference = 'Stop'

function Assert-PlainPath([string]$Path, [bool]$Directory) {
    if (-not $Path) { throw 'REQUIRED_PATH_MISSING' }
    $full = [IO.Path]::GetFullPath($ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Path))
    $item = Get-Item -LiteralPath $full -Force -ErrorAction Stop
    if ($item.PSIsContainer -ne $Directory -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'PATH_TYPE_OR_LINK_INVALID'
    }
    # FileInfo has Directory rather than Parent; inspect every ancestor too.
    $cursor = if ($Directory) { $item } else { $item.Directory }
    while ($cursor) {
        if ($cursor.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'PATH_REPARSE_POINT' }
        $cursor = $cursor.Parent
    }
    return $full
}

if (-not $IsWindows) { throw 'WINDOWS_REQUIRED' }
if ($ExpectedCatalogSha256 -cnotmatch '^[A-Fa-f0-9]{64}$' -or
    $ExpectedSignerThumbprint -cnotmatch '^[A-Fa-f0-9]{40}$' -or
    $ExpectedRootThumbprint -cnotmatch '^[A-Fa-f0-9]{40}$' -or
    -not $ExpectedPublisherSubject -or $ExpectedPublisherSubject.Length -gt 512) {
    throw 'EXPECTED_TRUST_POLICY_REQUIRED'
}

$payload = Assert-PlainPath $PayloadDirectory $true
$catalog = Assert-PlainPath $CatalogPath $false
$signTool = Assert-PlainPath $SignToolPath $false
if ([IO.Path]::GetExtension($catalog) -ine '.cat' -or [IO.Path]::GetExtension($signTool) -ine '.exe' -or
    $catalog.StartsWith($payload + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or
    $signTool.StartsWith($payload + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'SIGNED_LAYOUT_INVALID'
}
if ((Get-FileHash -LiteralPath $catalog -Algorithm SHA256).Hash -ine $ExpectedCatalogSha256) {
    throw 'CATALOG_DIGEST_MISMATCH'
}

$pending = New-Object 'System.Collections.Generic.Queue[string]'
$pending.Enqueue($payload)
$files = New-Object 'System.Collections.Generic.List[string]'
while ($pending.Count -gt 0) {
    foreach ($item in (Get-ChildItem -LiteralPath $pending.Dequeue() -Force -ErrorAction Stop)) {
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'PAYLOAD_REPARSE_POINT' }
        if ($item.PSIsContainer) { $pending.Enqueue($item.FullName) }
        elseif ($item.Length -ge 0) { $files.Add($item.FullName) }
        else { throw 'PAYLOAD_ITEM_INVALID' }
    }
}
if ($files.Count -lt 4 -or $files.Count -gt 4096) { throw 'PAYLOAD_FILE_COUNT_INVALID' }
foreach ($required in @('relay.exe', 'relayd.exe', 'LICENSE', 'NOTICE-RELAY.txt')) {
    if (-not ($files.Contains((Join-Path $payload $required)))) { throw 'REQUIRED_PAYLOAD_FILE_MISSING' }
}

# Windows verifies every relative payload hash against the detached catalog.
$comparison = Test-FileCatalog -CatalogFilePath $catalog -Path $payload -Detailed -ErrorAction Stop
if ([string]$comparison.Status -cne 'Valid' -or [string]$comparison.HashAlgorithm -cne 'SHA256' -or
    $comparison.CatalogItems.Count -ne $files.Count -or $comparison.PathItems.Count -ne $files.Count) {
    throw 'CATALOG_PAYLOAD_MISMATCH'
}

# Authenticode must be trusted and match the separately reviewed publisher policy.
$signature = Get-AuthenticodeSignature -LiteralPath $catalog -ErrorAction Stop
if ([string]$signature.Status -cne 'Valid' -or -not $signature.SignerCertificate) {
    throw 'CATALOG_SIGNATURE_UNTRUSTED'
}
$signer = $signature.SignerCertificate
if ($signer.Subject -cne $ExpectedPublisherSubject -or
    $signer.Thumbprint -ine $ExpectedSignerThumbprint) { throw 'PUBLISHER_IDENTITY_MISMATCH' }

# A current, non-revoked code-signing chain must terminate at the pinned root.
$chain = New-Object Security.Cryptography.X509Certificates.X509Chain
try {
    $chain.ChainPolicy.RevocationMode = [Security.Cryptography.X509Certificates.X509RevocationMode]::Online
    $chain.ChainPolicy.RevocationFlag = [Security.Cryptography.X509Certificates.X509RevocationFlag]::EntireChain
    $chain.ChainPolicy.VerificationFlags = [Security.Cryptography.X509Certificates.X509VerificationFlags]::NoFlag
    $chain.ChainPolicy.TrustMode = [Security.Cryptography.X509Certificates.X509ChainTrustMode]::System
    $chain.ChainPolicy.UrlRetrievalTimeout = [TimeSpan]::FromSeconds(15)
    $null = $chain.ChainPolicy.ApplicationPolicy.Add([Security.Cryptography.Oid]::new('1.3.6.1.5.5.7.3.3'))
    if (-not $chain.Build($signer) -or $chain.ChainElements.Count -lt 2) {
        throw 'PUBLISHER_CHAIN_UNTRUSTED'
    }
    $root = $chain.ChainElements[$chain.ChainElements.Count - 1].Certificate
    if ($root.Thumbprint -ine $ExpectedRootThumbprint -or $root.Thumbprint -ieq $signer.Thumbprint) {
        throw 'PUBLISHER_ROOT_MISMATCH'
    }
} finally { $chain.Dispose() }

# SignTool applies the Windows Authenticode policy and requires a timestamp.
# Exit 2 is a warning and is deliberately a release failure.
$toolSignature = Get-AuthenticodeSignature -LiteralPath $signTool -ErrorAction Stop
if ([string]$toolSignature.Status -cne 'Valid' -or -not $toolSignature.SignerCertificate -or
    $toolSignature.SignerCertificate.Subject -notmatch '(^|, )O=Microsoft Corporation(,|$)') {
    throw 'SIGNTOOL_BINARY_UNTRUSTED'
}
$PSNativeCommandUseErrorActionPreference = $false
& $signTool verify /pa /all /tw $catalog 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'SIGNTOOL_TRUST_OR_TIMESTAMP_FAILED' }

Write-Output "SIGNED_DISTRIBUTION_VERIFIED files=$($files.Count) catalog_sha256=$($ExpectedCatalogSha256.ToLowerInvariant())"
