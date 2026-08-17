[CmdletBinding()]
param([switch]$SkipClippy)

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
$manifest = Join-Path $repoRoot 'Cargo.toml'
if (-not (Test-Path -LiteralPath $manifest -PathType Leaf)) {
    throw "Could not resolve Alchetron repository from $PSScriptRoot"
}

Push-Location $repoRoot
$scratch = Join-Path ([System.IO.Path]::GetTempPath()) ("alchetron-verify-" + [guid]::NewGuid().ToString('N'))
try {
    New-Item -ItemType Directory -Path $scratch | Out-Null
    Write-Host 'ALCHETRON / VERIFY' -ForegroundColor Cyan
    rustc --version
    cargo --version
    Get-ChildItem -LiteralPath (Join-Path $repoRoot 'schemas') -Filter '*.json' |
        ForEach-Object { Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json | Out-Null }
    Get-Content -LiteralPath (Join-Path $repoRoot 'registry\agents.json') -Raw | ConvertFrom-Json | Out-Null
    Get-Content -LiteralPath (Join-Path $repoRoot 'registry\tools.json') -Raw | ConvertFrom-Json | Out-Null
    Get-Content -LiteralPath (Join-Path $repoRoot 'registry\permissions.json') -Raw | ConvertFrom-Json | Out-Null
    cargo fmt --all -- --check
    cargo check --locked --all-targets
    if (-not $SkipClippy) { cargo clippy --locked --all-targets -- -D warnings }
    cargo test --locked --all-targets
    cargo run --locked --bin algo -- --root $repoRoot --data-dir $scratch smoke --process
    cargo run --locked --bin algo -- --root $repoRoot --data-dir $scratch supervisor
    cargo run --locked --bin algo -- --root $repoRoot --data-dir $scratch graph snapshot
    Write-Host 'VERDICT: PASS' -ForegroundColor Green
}
finally {
    Pop-Location
    if (Test-Path -LiteralPath $scratch) {
        $resolvedScratch = (Resolve-Path -LiteralPath $scratch).Path
        $resolvedTemp = (Resolve-Path -LiteralPath ([System.IO.Path]::GetTempPath())).Path
        if ($resolvedScratch.StartsWith($resolvedTemp, [System.StringComparison]::OrdinalIgnoreCase)) {
            Remove-Item -LiteralPath $resolvedScratch -Recurse -Force
        }
    }
}
