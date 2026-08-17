[CmdletBinding()]
param(
    [string]$Bind = '127.0.0.1:8791',
    [switch]$NoOpen,
    [switch]$Verify
)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path -LiteralPath (Join-Path $repoRoot 'Cargo.toml') -PathType Leaf)) {
    throw "Could not resolve Alchetron repository from $PSScriptRoot"
}
Push-Location $repoRoot
try {
    if ($Verify) { & (Join-Path $PSScriptRoot 'verify.ps1') -SkipClippy }
    $arguments = @('run', '--locked', '--bin', 'algo', '--', '--root', $repoRoot, 'ui', '--bind', $Bind)
    if ($NoOpen) { $arguments += '--no-open' }
    & cargo @arguments
}
finally {
    Pop-Location
}
