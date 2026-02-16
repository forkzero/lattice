# Lattice installer for Windows
# Usage: irm https://forkzero.ai/lattice/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "forkzero/lattice"
$InstallDir = if ($env:INSTALL_DIR) { $env:INSTALL_DIR } else { "$env:LOCALAPPDATA\lattice" }

# Detect architecture
$Arch = $env:PROCESSOR_ARCHITECTURE
switch ($Arch) {
    "AMD64" { $Target = "x86_64-pc-windows-msvc" }
    default { Write-Error "Unsupported architecture: $Arch"; exit 1 }
}

# Get latest version
if ($env:VERSION) {
    $Version = $env:VERSION
} else {
    $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $Version = $Release.tag_name -replace '^v', ''
}

if (-not $Version) {
    Write-Error "Failed to detect latest version"
    exit 1
}

$Archive = "lattice-$Version-$Target.tar.gz"
$Url = "https://github.com/$Repo/releases/download/v$Version/$Archive"
$ChecksumUrl = "https://github.com/$Repo/releases/download/v$Version/checksums.txt"

Write-Host "Installing lattice v$Version for $Target..."

# Create temp directory
$TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) "lattice-install-$(Get-Random)"
New-Item -ItemType Directory -Path $TmpDir -Force | Out-Null

try {
    # Download archive
    Write-Host "Downloading $Url..."
    Invoke-WebRequest -Uri $Url -OutFile "$TmpDir\$Archive" -UseBasicParsing

    # Download checksums
    Write-Host "Verifying checksum..."
    Invoke-WebRequest -Uri $ChecksumUrl -OutFile "$TmpDir\checksums.txt" -UseBasicParsing

    # Verify checksum
    $ExpectedLine = Get-Content "$TmpDir\checksums.txt" | Where-Object { $_ -match $Archive }
    if ($ExpectedLine) {
        $ExpectedHash = ($ExpectedLine -split '\s+')[0]
        $ActualHash = (Get-FileHash -Path "$TmpDir\$Archive" -Algorithm SHA256).Hash.ToLower()
        if ($ActualHash -ne $ExpectedHash) {
            Write-Error "Checksum mismatch! Expected: $ExpectedHash, Got: $ActualHash"
            exit 1
        }
    } else {
        Write-Warning "Could not find checksum for $Archive"
    }

    # Extract
    Write-Host "Extracting..."
    tar -xzf "$TmpDir\$Archive" -C $TmpDir

    # Install
    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }
    Copy-Item "$TmpDir\lattice-$Version-$Target\lattice.exe" -Destination "$InstallDir\lattice.exe" -Force

    # Verify
    $OnPath = $env:PATH -split ';' | Where-Object { $_ -eq $InstallDir }
    if ($OnPath) {
        Write-Host ""
        Write-Host "Successfully installed lattice v$Version"
        Write-Host ""
        Write-Host "Get started:"
        Write-Host "  lattice init          # Initialize a lattice"
        Write-Host "  lattice --help        # Show all commands"
    } else {
        Write-Host ""
        Write-Host "Installed to $InstallDir\lattice.exe"
        Write-Host ""
        Write-Host "Add to your PATH by running:"
        Write-Host "  `$env:PATH += `";$InstallDir`""
        Write-Host ""
        Write-Host "To make it permanent, run:"
        Write-Host "  [Environment]::SetEnvironmentVariable('PATH', `$env:PATH + ';$InstallDir', 'User')"
    }
} finally {
    # Clean up temp directory
    Remove-Item -Path $TmpDir -Recurse -Force -ErrorAction SilentlyContinue
}
