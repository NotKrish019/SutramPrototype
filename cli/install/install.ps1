# YUKTI installer for Windows (PowerShell 5.1+ or PowerShell 7).
#
#   irm https://raw.githubusercontent.com/NotKrish019/SutramPrototype/main/cli/install/install.ps1 | iex
#
# Installs yukti.exe and the bundled yukti-server.exe into %LOCALAPPDATA%\yukti\bin, verifies the
# download's SHA-256, and adds the folder to your user PATH. Then open a new terminal and run: yukti
#
# Environment:
#   YUKTI_VERSION      release tag, e.g. yukti-v0.1.0 (default: latest)
#   YUKTI_INSTALL_DIR  where the programs go (default: %LOCALAPPDATA%\yukti\bin)
#   YUKTI_REPO         GitHub owner/repo (default: NotKrish019/SutramPrototype)
#   YUKTI_ARCHIVE      install from this local .zip instead of downloading (testing)
#   YUKTI_NO_MODIFY_PATH=1  do not change the user PATH

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'   # Invoke-WebRequest is much faster without the bar

function Get-Setting([string]$Name, [string]$Default) {
    $v = [Environment]::GetEnvironmentVariable($Name)
    if ([string]::IsNullOrWhiteSpace($v)) { return $Default }
    return $v
}

$Repo       = Get-Setting 'YUKTI_REPO' 'NotKrish019/SutramPrototype'
$Version    = Get-Setting 'YUKTI_VERSION' 'latest'
$InstallDir = Get-Setting 'YUKTI_INSTALL_DIR' (Join-Path $env:LOCALAPPDATA 'yukti\bin')

function Step([string]$Text) { Write-Host '  ' -NoNewline; Write-Host ([char]0x258C) -ForegroundColor Red -NoNewline; Write-Host " $Text" }
function Fail([string]$Text) { Write-Host '  error: ' -ForegroundColor Red -NoNewline; Write-Host $Text; throw "YUKTI installation failed: $Text" }

Write-Host ''
Write-Host '  ' -NoNewline; Write-Host "$([char]0x258C) YUKTI" -ForegroundColor Red -NoNewline
Write-Host '  Digital Evidence Correlation & Crime Reconstruction' -ForegroundColor DarkGray
Write-Host ''

# --- platform -------------------------------------------------------------------------------
$arch = $env:PROCESSOR_ARCHITECTURE
if ($env:PROCESSOR_ARCHITEW6432) { $arch = $env:PROCESSOR_ARCHITEW6432 }
switch ($arch) {
    'AMD64' { $target = 'x86_64-pc-windows-msvc' }
    # Windows on ARM runs x64 programs through emulation.
    'ARM64' { $target = 'x86_64-pc-windows-msvc' }
    default { Fail "unsupported CPU architecture: $arch" }
}
$asset = "yukti-$target.zip"
Step "Platform $target"

# --- download -------------------------------------------------------------------------------
$tmp = Join-Path ([IO.Path]::GetTempPath()) ("yukti-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    $zip = Join-Path $tmp $asset
    $sumFile = "$zip.sha256"
    $local = Get-Setting 'YUKTI_ARCHIVE' ''
    if ($local) {
        if (-not (Test-Path $local)) { Fail "YUKTI_ARCHIVE not found: $local" }
        Copy-Item $local $zip
        if (Test-Path "$local.sha256") { Copy-Item "$local.sha256" $sumFile }
        Step "Using local archive $local"
    } else {
        if ($Version -eq 'latest') { $base = "https://github.com/$Repo/releases/latest/download" }
        else { $base = "https://github.com/$Repo/releases/download/$Version" }
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
        Step "Downloading $asset ($Version)"
        try {
            Invoke-WebRequest -Uri "$base/$asset" -OutFile $zip -UseBasicParsing
            Invoke-WebRequest -Uri "$base/$asset.sha256" -OutFile $sumFile -UseBasicParsing
        } catch {
            Fail "download failed from $base/$asset (is there a published release?): $($_.Exception.Message)"
        }
    }

    # --- verify -----------------------------------------------------------------------------
    $actual = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
    if (Test-Path $sumFile) {
        $expected = ((Get-Content $sumFile -Raw).Trim() -split '\s+')[0].ToLower()
        if ($expected -ne $actual) { Fail "SHA-256 mismatch: expected $expected, got $actual. Nothing was installed." }
        Step "SHA-256 verified $actual"
    } elseif (-not $local) {
        Fail 'no checksum available; refusing to install'
    } else {
        Write-Host '    (local archive without .sha256: checksum not verified)' -ForegroundColor DarkGray
    }

    # --- install ----------------------------------------------------------------------------
    Expand-Archive -Path $zip -DestinationPath $tmp -Force
    $src = Join-Path $tmp "yukti-$target"
    foreach ($bin in 'yukti.exe', 'yukti-server.exe') {
        if (-not (Test-Path (Join-Path $src $bin))) { Fail "archive does not contain $bin" }
    }
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    $existing = Join-Path $InstallDir 'yukti.exe'
    if (Test-Path $existing) {
        # A running server from an older install would keep its .exe locked.
        try { & $existing server stop *> $null } catch { }
        Get-Process -Name 'yukti-server' -ErrorAction SilentlyContinue |
            Where-Object { $_.Path -eq (Join-Path $InstallDir 'yukti-server.exe') } |
            Stop-Process -Force -ErrorAction SilentlyContinue
    }
    foreach ($bin in 'yukti.exe', 'yukti-server.exe', 'README.txt') {
        $from = Join-Path $src $bin
        if (Test-Path $from) { Copy-Item $from (Join-Path $InstallDir $bin) -Force }
    }
    Get-ChildItem $InstallDir -Filter *.exe | Unblock-File -ErrorAction SilentlyContinue
    Step "Installed to $InstallDir"

    # --- PATH -------------------------------------------------------------------------------
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($null -eq $userPath) { $userPath = '' }
    $parts = $userPath -split ';' | Where-Object { $_ -ne '' }
    if (($parts -notcontains $InstallDir) -and ((Get-Setting 'YUKTI_NO_MODIFY_PATH' '0') -ne '1')) {
        [Environment]::SetEnvironmentVariable('Path', (($parts + $InstallDir) -join ';'), 'User')
        Step 'Added to your user PATH'
    }
    if (($env:Path -split ';') -notcontains $InstallDir) { $env:Path = "$env:Path;$InstallDir" }

    $version = & (Join-Path $InstallDir 'yukti.exe') --version
    Write-Host ''
    Write-Host '  ' -NoNewline; Write-Host ([char]0x2713) -ForegroundColor Green -NoNewline; Write-Host " $version installed."
    Write-Host ''
    Write-Host '  Next, open a new terminal and run:'
    Write-Host ''
    Write-Host '      yukti' -ForegroundColor White
    Write-Host ''
    Write-Host "  It starts the local YUKTI server and opens the interactive session. Type 'help' there." -ForegroundColor DarkGray
    Write-Host "  Uninstall: yukti server stop, then delete $InstallDir (case data stays in $env:LOCALAPPDATA\yukti\data)" -ForegroundColor DarkGray
    Write-Host ''
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
