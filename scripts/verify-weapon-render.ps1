[CmdletBinding()]
param(
    [string]$GameDir = $env:XIV_GAME_DIR
)

$ErrorActionPreference = "Stop"

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)]
        [string]$FilePath,
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$FilePath exited with code $LASTEXITCODE"
    }
}

$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $repoRoot

if ([string]::IsNullOrWhiteSpace($GameDir)) {
    $defaultGameDir = "E:\_ff14\game"
    if (Test-Path -LiteralPath $defaultGameDir -PathType Container) {
        $GameDir = $defaultGameDir
    } else {
        throw "Set XIV_GAME_DIR or pass -GameDir with an installed FFXIV game directory."
    }
}

$resolvedGameDir = (Resolve-Path -LiteralPath $GameDir -ErrorAction Stop).Path
$fixturePath = Join-Path $repoRoot "tests\fixtures\phantom_weapons.json"
$fixture = Get-Content -LiteralPath $fixturePath -Raw | ConvertFrom-Json
$priorityCases = @(
    $fixture.cases |
        Where-Object { $_.priority -in @("P0", "P1") } |
        ForEach-Object { $_.caseId }
)
if ($priorityCases.Count -eq 0) {
    throw "No P0/P1 phantom cases were found in $fixturePath."
}

$env:XIV_GAME_DIR = $resolvedGameDir
$env:XIV_PHANTOM_CASES = $priorityCases -join ","

Write-Host "FFXIV game directory: $resolvedGameDir"
Write-Host "P0/P1 phantom cases ($($priorityCases.Count)): $($priorityCases -join ', ')"

Invoke-Checked cargo @(
    "test", "--jobs", "1", "--features", "game-data", "--test", "weapon_shader_family_audit",
    "audit_installed_weapon_shader_families", "--", "--ignored", "--exact", "--nocapture"
)
Invoke-Checked cargo @(
    "test", "--jobs", "1", "--features", "game-data,render-test-support", "--test", "phantom_weapon_snapshots",
    "render_phantom_weapon_snapshots", "--", "--ignored", "--exact", "--nocapture"
)
Invoke-Checked cargo @(
    "test", "--jobs", "1", "--workspace", "--all-features", "--exclude", "xtask-update-craft-data"
)
Invoke-Checked cargo @(
    "check", "--jobs", "1", "--workspace", "--all-features", "--target", "wasm32-unknown-unknown"
)
Invoke-Checked cargo @("fmt", "--all", "--", "--check")
Invoke-Checked git @("diff", "--check")

Write-Host "Weapon render verification passed."
