<#
.SYNOPSIS
  Download CCP's Static Data Export and build the prebuilt sde.sqlite that
  EVE Commander loads at startup.

.DESCRIPTION
  The full SDE is too large to ship in the repo, so each user generates it
  locally once. This script downloads CCP's SDE zip, finds the FSD YAML files,
  runs the `sde-tools` converter, and writes sde.sqlite into the app's data dir
  (%APPDATA%\eve-commander by default) where the app discovers it automatically.

  Re-run it whenever you want to refresh against a newer SDE.

.PARAMETER OutDir
  Where to write sde.sqlite. Defaults to $env:APPDATA\eve-commander (the app's
  data dir on Windows).

.PARAMETER SdeUrl
  Override the SDE download URL if CCP moves it.

.EXAMPLE
  pwsh -File scripts/build-sde.ps1
#>
[CmdletBinding()]
param(
    [string]$OutDir = (Join-Path $env:APPDATA "eve-commander"),
    [string]$SdeUrl = "https://eve-static-data-export.s3-eu-west-1.amazonaws.com/tranquility/sde.zip"
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot

# Windows PowerShell 5.1 defaults to TLS 1.0 and will fail the HTTPS download
# from S3 — force TLS 1.2. (No-op on PowerShell 7+.)
try { [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12 } catch {}

Write-Host "EVE Commander - SDE builder" -ForegroundColor Cyan
Write-Host "Output dir: $OutDir"

# 1. Download + extract the SDE into a temp working area.
$work = Join-Path ([System.IO.Path]::GetTempPath()) "eve-sde-build"
$zip = Join-Path $work "sde.zip"
$extract = Join-Path $work "sde"
New-Item -ItemType Directory -Force -Path $work | Out-Null

if (-not (Test-Path $zip)) {
    Write-Host "Downloading SDE (~100 MB) ..."
    # ProgressPreference=SilentlyContinue makes Invoke-WebRequest download an
    # order of magnitude faster on Windows PowerShell 5.1.
    $oldProgress = $ProgressPreference
    $ProgressPreference = "SilentlyContinue"
    try { Invoke-WebRequest -Uri $SdeUrl -OutFile $zip -UseBasicParsing }
    finally { $ProgressPreference = $oldProgress }
} else {
    Write-Host "Reusing cached download: $zip"
}

if (Test-Path $extract) { Remove-Item -Recurse -Force $extract }
Write-Host "Extracting ..."
Expand-Archive -Path $zip -DestinationPath $extract -Force

# 2. Locate the FSD YAML files (paths vary slightly between SDE releases, so
#    search by name rather than assuming a layout).
function Find-Yaml([string[]]$names) {
    foreach ($n in $names) {
        $hit = Get-ChildItem -Path $extract -Recurse -Filter $n -ErrorAction SilentlyContinue |
               Select-Object -First 1
        if ($hit) { return $hit.FullName }
    }
    return $null
}

$types        = Find-Yaml @("typeIDs.yaml", "types.yaml")
$typeMaterials= Find-Yaml @("typeMaterials.yaml")
$blueprints   = Find-Yaml @("blueprints.yaml")
$typeDogma    = Find-Yaml @("typeDogma.yaml")

if (-not $types) { throw "Could not find typeIDs.yaml / types.yaml in the SDE." }
Write-Host "types:         $types"
Write-Host "typeMaterials: $typeMaterials"
Write-Host "blueprints:    $blueprints"
Write-Host "typeDogma:     $typeDogma"

# 3. Build the converter once (release for speed on the large YAML).
Write-Host "Building sde-tools (release) ..."
Push-Location $repoRoot
try {
    cargo build -p sde-tools --release
    $exe = Join-Path $repoRoot "target/release/sde-convert.exe"
    if (-not (Test-Path $exe)) { $exe = Join-Path $repoRoot "target/release/sde-tools.exe" }

    New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
    $out = Join-Path $OutDir "sde.sqlite"

    $args = @("--out", $out, "--types", $types)
    if ($typeMaterials) { $args += @("--type-materials", $typeMaterials) }
    if ($blueprints)    { $args += @("--blueprints", $blueprints) }
    if ($typeDogma)     { $args += @("--type-dogma", $typeDogma) }

    Write-Host "Converting -> $out" -ForegroundColor Cyan
    & $exe @args
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "Done. Restart EVE Commander — it will load the prebuilt SDE and the" -ForegroundColor Green
Write-Host "reprocessing / build-planner / skill-plan / can-I-fly features go live." -ForegroundColor Green
