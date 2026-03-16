#Requires -Version 5.1
# zenmux-adapter installer for Windows (PowerShell)
#
# Usage:
#   irm https://raw.githubusercontent.com/aitiotekt/zenmux-adapter/main/scripts/install.ps1 | iex
#
# Or with explicit TLS 1.2:
#   [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
#   irm https://raw.githubusercontent.com/aitiotekt/zenmux-adapter/main/scripts/install.ps1 | iex

$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$Repo       = "aitiotekt/zenmux-adapter"
$Binary     = "zenmux-adapter"
$InstallDir = Join-Path $env:USERPROFILE ".local\bin"

# ── Helpers ──────────────────────────────────────────────────────────────────
function Write-Info    ($msg) { Write-Host "info:    $msg" -ForegroundColor Cyan    }
function Write-Ok      ($msg) { Write-Host "ok:      $msg" -ForegroundColor Green   }
function Write-Warn    ($msg) { Write-Host "warning: $msg" -ForegroundColor Yellow  }
function Write-Err     ($msg) { Write-Host "error:   $msg" -ForegroundColor Red; exit 1 }

# ── Platform detection ───────────────────────────────────────────────────────
function Get-Arch {
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    switch ($arch) {
        'X64'   { return 'x86_64'  }
        'Arm64' { return 'aarch64' }
        default { Write-Err "Unsupported architecture: $arch" }
    }
}

function Get-Target ($arch) {
    switch ($arch) {
        'x86_64'  { return 'x86_64-pc-windows-msvc'  }
        'aarch64' { return 'aarch64-pc-windows-msvc' }
        default   { Write-Err "No pre-built binary for Windows/$arch" }
    }
}

# ── Version resolution ───────────────────────────────────────────────────────
function Get-LatestVersion {
    try {
        $resp = Invoke-RestMethod `
            -Uri "https://api.github.com/repos/$Repo/releases/latest" `
            -ErrorAction Stop
        return $resp.tag_name
    } catch {
        Write-Err "Failed to fetch the latest release version: $_"
    }
}

# ── Main ─────────────────────────────────────────────────────────────────────
function Main {
    Write-Host ""
    Write-Host "Installing $Binary" -ForegroundColor White -BackgroundColor DarkBlue
    Write-Host ""

    Write-Info "Detecting platform..."
    $arch   = Get-Arch
    $target = Get-Target $arch
    Write-Info "Platform : windows / $arch"
    Write-Info "Target   : $target"

    Write-Info "Fetching latest release..."
    $version = Get-LatestVersion
    Write-Info "Version  : $version"

    $filename = "$Binary-$version-$target.zip"
    $url      = "https://github.com/$Repo/releases/download/$version/$filename"

    Write-Info "Downloading $filename ..."
    $tmpDir  = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid())
    New-Item -ItemType Directory -Path $tmpDir | Out-Null

    try {
        $zipPath = Join-Path $tmpDir $filename
        Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing

        Write-Info "Extracting..."
        Expand-Archive -Path $zipPath -DestinationPath $tmpDir -Force

        Write-Info "Installing to $InstallDir\$Binary.exe ..."
        if (-not (Test-Path $InstallDir)) {
            New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        }

        $exeSrc = Join-Path $tmpDir "$Binary.exe"
        $exeDst = Join-Path $InstallDir "$Binary.exe"
        Copy-Item -Path $exeSrc -Destination $exeDst -Force

        Write-Host ""
        Write-Ok "Installed $Binary $version → $exeDst"

        # PATH hint
        $userPath = [System.Environment]::GetEnvironmentVariable('PATH', 'User')
        if ($userPath -notlike "*$InstallDir*") {
            Write-Host ""
            Write-Warn "$InstallDir is not in your PATH."
            Write-Warn "To add it permanently, run the following in a new terminal:"
            Write-Warn "  `$env:PATH = `"$InstallDir;`$env:PATH`""
            Write-Warn "  [System.Environment]::SetEnvironmentVariable('PATH', `"$InstallDir;`$userPath`", 'User')"
        }

        Write-Host ""
        Write-Info "Run ``$Binary --help`` to get started."
        Write-Host ""
    } finally {
        Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
    }
}

Main
