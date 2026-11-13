# Installs Stubprint @VERSION@ for Windows: it downloads the archive from the release, checks it
# against SHA256SUMS and puts stubprint.exe in a folder that it adds to your PATH.
#
#   irm https://stubprint.dev/install.ps1 | iex
#
# $env:STUBPRINT_INSTALL_DIR   where to put it, by default %LOCALAPPDATA%\Programs\stubprint
# $env:STUBPRINT_DOWNLOAD_URL  where the files of the release are, for a mirror or a test
#
# The release attaches this script with the version filled in, so it installs that version.
$ErrorActionPreference = 'Stop'
# The progress bar of Invoke-WebRequest makes downloads many times slower.
$ProgressPreference = 'SilentlyContinue'

$version = '@VERSION@'
$repo = 'Wadiou/stubprint'

if ($version.StartsWith('@')) {
    throw 'This is the template in the repository: use the installer attached to a release.'
}
if (-not [Environment]::Is64BitOperatingSystem) {
    throw "There is no 32 bit build: see https://github.com/$repo/releases"
}

$base = if ($env:STUBPRINT_DOWNLOAD_URL) { $env:STUBPRINT_DOWNLOAD_URL } else { "https://github.com/$repo/releases/download/v$version" }
$dir = if ($env:STUBPRINT_INSTALL_DIR) { $env:STUBPRINT_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\stubprint' }
$name = "stubprint-$version-x86_64-pc-windows-msvc"
$archive = "$name.zip"

$work = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $work | Out-Null
try {
    Write-Host "==> " -ForegroundColor Cyan -NoNewline
    Write-Host "Downloading Stubprint $version" -NoNewline
    Write-Host " for x86_64-pc-windows-msvc" -ForegroundColor DarkGray
    Invoke-WebRequest -Uri "$base/$archive" -OutFile (Join-Path $work $archive)
    Invoke-WebRequest -Uri "$base/SHA256SUMS" -OutFile (Join-Path $work 'SHA256SUMS')

    $line = Get-Content (Join-Path $work 'SHA256SUMS') | Where-Object { $_ -match "  $([regex]::Escape($archive))$" } | Select-Object -First 1
    if (-not $line) { throw "SHA256SUMS has no line for $archive" }
    $expected = ($line -split '\s+')[0]
    $actual = (Get-FileHash -Algorithm SHA256 (Join-Path $work $archive)).Hash
    if ($actual -ne $expected.ToUpperInvariant()) { throw 'The download does not match its checksum.' }

    Expand-Archive -Path (Join-Path $work $archive) -DestinationPath $work -Force
    New-Item -ItemType Directory -Path $dir -Force | Out-Null
    Copy-Item (Join-Path $work "$name\stubprint.exe") (Join-Path $dir 'stubprint.exe') -Force
}
finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}

Write-Host "$([char]0x2713) " -ForegroundColor Green -NoNewline
Write-Host "Installed $dir\stubprint.exe"
# The PATH of this user only (no admin rights needed). This terminal gets it too, so stubprint
# runs at once here; terminals that are already open need to be reopened.
$user = [Environment]::GetEnvironmentVariable('Path', 'User')
if (($user -split ';') -notcontains $dir) {
    [Environment]::SetEnvironmentVariable('Path', "$user;$dir".TrimStart(';'), 'User')
    Write-Host "! " -ForegroundColor Yellow -NoNewline
    Write-Host "Added $dir to your PATH: other terminals that are open need to be reopened."
}
if (($env:Path -split ';') -notcontains $dir) { $env:Path = "$env:Path;$dir" }
Write-Host ""
Write-Host "Next, run " -NoNewline
Write-Host "stubprint" -ForegroundColor Cyan
