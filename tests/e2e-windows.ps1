# Lattice Windows E2E Smoke Test
# Validates install script works and binary runs on Windows

$ErrorActionPreference = "Stop"

$Pass = 0
$Fail = 0

function Test-Pass($msg) { $script:Pass++; Write-Host "  + $msg" }
function Test-Fail($msg) { $script:Fail++; Write-Host "  x $msg" }

function Assert-Contains($output, $expected, $msg) {
    if ($output -match [regex]::Escape($expected)) { Test-Pass $msg }
    else { Test-Fail "$msg (expected '$expected')" }
}

Write-Host ""
Write-Host "Lattice Windows E2E Smoke Test"
Write-Host "=============================="
Write-Host ""

# === Verify binary ===
Write-Host "--- Binary ---"

$version = & lattice --version 2>&1
if ($LASTEXITCODE -eq 0) { Test-Pass "lattice --version ($version)" }
else { Test-Fail "lattice --version failed"; exit 1 }

# === Init ===
Write-Host ""
Write-Host "--- Init ---"

$workdir = Join-Path ([System.IO.Path]::GetTempPath()) "lattice-e2e-$(Get-Random)"
New-Item -ItemType Directory -Path $workdir -Force | Out-Null
Push-Location $workdir

try {
    git init -q
    git config user.email "e2e@test.local"
    git config user.name "E2E Test"
    git remote add origin https://github.com/test/test.git

    & lattice init
    if (Test-Path ".lattice") { Test-Pass ".lattice directory created" }
    else { Test-Fail ".lattice directory missing" }

    if (Test-Path ".lattice\config.yaml") { Test-Pass "config.yaml exists" }
    else { Test-Fail "config.yaml missing" }

    # === Add nodes ===
    Write-Host ""
    Write-Host "--- Add Nodes ---"

    & lattice add source `
        --id SRC-WIN-001 `
        --title "Windows Test Source" `
        --body "Testing on Windows" `
        --created-by "human:e2e-windows"
    if ($LASTEXITCODE -eq 0) { Test-Pass "Added SRC-WIN-001" }
    else { Test-Fail "Failed to add SRC-WIN-001" }

    & lattice add thesis `
        --id THX-WIN-001 `
        --title "Windows Support Matters" `
        --body "Cross-platform is important" `
        --category technical `
        --confidence 0.9 `
        --supported-by SRC-WIN-001 `
        --created-by "human:e2e-windows"
    if ($LASTEXITCODE -eq 0) { Test-Pass "Added THX-WIN-001" }
    else { Test-Fail "Failed to add THX-WIN-001" }

    & lattice add requirement `
        --id REQ-WIN-001 `
        --title "Windows Binary" `
        --body "Must build and run on Windows" `
        --priority P0 `
        --category PLATFORM `
        --derives-from THX-WIN-001 `
        --created-by "human:e2e-windows"
    if ($LASTEXITCODE -eq 0) { Test-Pass "Added REQ-WIN-001" }
    else { Test-Fail "Failed to add REQ-WIN-001" }

    # === List / Get ===
    Write-Host ""
    Write-Host "--- List and Get ---"

    $out = & lattice list sources
    Assert-Contains $out "SRC-WIN-001" "list sources: SRC-WIN-001"

    $out = & lattice list requirements
    Assert-Contains $out "REQ-WIN-001" "list requirements: REQ-WIN-001"

    $out = & lattice get REQ-WIN-001
    Assert-Contains $out "Windows Binary" "get REQ-WIN-001"

    # === Summary ===
    Write-Host ""
    Write-Host "--- Summary ---"

    $out = & lattice summary
    Assert-Contains $out "source" "summary: shows sources"
    Assert-Contains $out "requirement" "summary: shows requirements"

    # === Export JSON ===
    Write-Host ""
    Write-Host "--- Export ---"

    $out = & lattice export --format json
    Assert-Contains $out "SRC-WIN-001" "JSON export: contains source"
    Assert-Contains $out "REQ-WIN-001" "JSON export: contains requirement"

    # === Drift ===
    Write-Host ""
    Write-Host "--- Drift ---"

    & lattice drift --check
    if ($LASTEXITCODE -eq 0) { Test-Pass "drift --check exits 0" }
    else { Test-Fail "drift --check should exit 0" }

} finally {
    Pop-Location
    Remove-Item -Path $workdir -Recurse -Force -ErrorAction SilentlyContinue
}

# === Results ===
Write-Host ""
Write-Host "=============================="
Write-Host "Results: $Pass passed, $Fail failed"
Write-Host ""

if ($Fail -gt 0) { exit 1 } else { exit 0 }
