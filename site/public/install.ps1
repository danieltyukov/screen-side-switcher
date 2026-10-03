# Installs the screen-side command on Windows.
#
#   powershell -ExecutionPolicy ByPass -c "irm https://danieltyukov.github.io/screen-side-switcher/install.ps1 | iex"
#
# It downloads the archive for this machine from the latest GitHub release,
# checks it against the release's SHA256SUMS, puts screen-side.exe in
# %LOCALAPPDATA%\Programs\screen-side and adds that folder to your PATH.
#
# SCREEN_SIDE_INSTALL_DIR  where to put it
# SCREEN_SIDE_VERSION      a version such as 2.0.0 (default: the latest)
# SCREEN_SIDE_ARCHIVE      a local .zip to install instead of downloading

$ErrorActionPreference = 'Stop'
$Repo = 'danieltyukov/screen-side-switcher'

function Fail($Message) {
    Write-Error "screen-side install: $Message"
    exit 1
}

$Arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
$Asset = switch ($Arch) {
    'AMD64' { 'screen-side-windows-x64.zip' }
    'ARM64' { 'screen-side-windows-arm64.zip' }
    default { Fail "there is no build for Windows on $Arch yet." }
}

$Version = if ($env:SCREEN_SIDE_VERSION) { $env:SCREEN_SIDE_VERSION } else { 'latest' }
$Base = if ($Version -eq 'latest') {
    "https://github.com/$Repo/releases/latest/download"
} else {
    "https://github.com/$Repo/releases/download/v$($Version.TrimStart('v'))"
}
$Dest = if ($env:SCREEN_SIDE_INSTALL_DIR) { $env:SCREEN_SIDE_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\screen-side' }

$Tmp = Join-Path ([IO.Path]::GetTempPath()) ("screen-side-" + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $Tmp | Out-Null
try {
    if ($env:SCREEN_SIDE_ARCHIVE) {
        $Name = Split-Path $env:SCREEN_SIDE_ARCHIVE -Leaf
        Copy-Item $env:SCREEN_SIDE_ARCHIVE (Join-Path $Tmp $Name)
        $Sums = Join-Path (Split-Path $env:SCREEN_SIDE_ARCHIVE -Parent) 'SHA256SUMS'
        if (Test-Path $Sums) { Copy-Item $Sums (Join-Path $Tmp 'SHA256SUMS') }
    } else {
        $Name = $Asset
        Write-Host "Downloading $Name"
        Invoke-WebRequest -UseBasicParsing "$Base/$Name" -OutFile (Join-Path $Tmp $Name)
        Invoke-WebRequest -UseBasicParsing "$Base/SHA256SUMS" -OutFile (Join-Path $Tmp 'SHA256SUMS')
    }

    # The archive must match the release's checksum before anything is installed.
    $SumsFile = Join-Path $Tmp 'SHA256SUMS'
    if (Test-Path $SumsFile) {
        $Line = Get-Content $SumsFile | Where-Object { ($_ -split '\s+')[1].TrimStart('*') -eq $Name } | Select-Object -First 1
        if (-not $Line) { Fail "SHA256SUMS has no line for $Name." }
        $Expected = ($Line -split '\s+')[0].ToLower()
        $Actual = (Get-FileHash -Algorithm SHA256 (Join-Path $Tmp $Name)).Hash.ToLower()
        if ($Expected -ne $Actual) { Fail "the checksum of $Name does not match the release. Nothing was installed." }
    }

    $Unpacked = Join-Path $Tmp 'x'
    Expand-Archive -Path (Join-Path $Tmp $Name) -DestinationPath $Unpacked
    $Exe = Join-Path $Unpacked 'screen-side.exe'
    if (-not (Test-Path $Exe)) { Fail "$Name does not contain screen-side.exe." }

    New-Item -ItemType Directory -Force -Path $Dest | Out-Null
    Copy-Item $Exe (Join-Path $Dest 'screen-side.exe') -Force
    $Installed = & (Join-Path $Dest 'screen-side.exe') --version
    Write-Host "Installed $Installed to $Dest"

    $UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (-not (($UserPath -split ';') -contains $Dest)) {
        [Environment]::SetEnvironmentVariable('Path', ($(if ($UserPath) { "$UserPath;" } else { '' }) + $Dest), 'User')
        Write-Host "Added $Dest to your PATH. Open a new terminal to use screen-side."
    }
    Write-Host 'Try: screen-side status'
    Write-Host "The app with the window and tray icon is ScreenSide_x64-setup.exe at https://github.com/$Repo/releases/latest"
} finally {
    Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
}
