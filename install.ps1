[CmdletBinding()]
param(
    [string]$Version = 'latest',
    [string]$InstallRoot = (Join-Path $env:LOCALAPPDATA 'Alchetron')
)

$ErrorActionPreference = 'Stop'
$repository = 'jacksonjp0311-gif/algorithm-agents'
$release = if ($Version -eq 'latest') {
    Invoke-RestMethod "https://api.github.com/repos/$repository/releases/latest"
} else {
    Invoke-RestMethod "https://api.github.com/repos/$repository/releases/tags/$Version"
}
$asset = $release.assets | Where-Object { $_.name -eq 'alchetron-windows-x86_64.zip' } | Select-Object -First 1
if (-not $asset) { throw 'The release does not contain the Windows runtime package' }
New-Item -ItemType Directory -Path $InstallRoot -Force | Out-Null
$temporary = Join-Path ([System.IO.Path]::GetTempPath()) ("alchetron-" + [guid]::NewGuid().ToString('N') + '.zip')
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $temporary
Expand-Archive -LiteralPath $temporary -DestinationPath $InstallRoot -Force
Remove-Item -LiteralPath $temporary -Force
$destination = Join-Path $InstallRoot 'algo.exe'
Write-Host "Alchetron installed at $destination" -ForegroundColor Cyan
Write-Host "Add $InstallRoot to PATH, then run: algo doctor"
