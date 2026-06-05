# Thoth installer / updater (Windows).
#
#   irm https://raw.githubusercontent.com/anitnilay20/thoth/main/install.ps1 | iex
#
# Re-running installs the latest release, so the same command updates Thoth.
$ErrorActionPreference = 'Stop'

$Repo    = 'anitnilay20/thoth'
$Target  = 'x86_64-pc-windows-msvc'
$Asset   = "thoth-$Target.zip"
$InstallDir = Join-Path $env:LOCALAPPDATA 'Programs\Thoth'

Write-Host '==> Finding the latest Thoth release...'
$release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" `
  -Headers @{ 'User-Agent' = 'thoth-installer' } -TimeoutSec 30
$tag = $release.tag_name
if (-not $tag) { throw 'Could not determine the latest release tag.' }
Write-Host "==> Latest release: $tag"

$url = "https://github.com/$Repo/releases/download/$tag/$Asset"
$tmp = Join-Path $env:TEMP ("thoth-" + [System.Guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
try {
  $zip = Join-Path $tmp $Asset
  Write-Host "==> Downloading $Asset"
  Invoke-WebRequest -Uri $url -OutFile $zip -TimeoutSec 300
  Expand-Archive -Path $zip -DestinationPath $tmp -Force

  $exe = Get-ChildItem -Path $tmp -Filter 'thoth.exe' -Recurse | Select-Object -First 1
  if (-not $exe) { throw "Archive did not contain thoth.exe" }

  New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
  Copy-Item $exe.FullName (Join-Path $InstallDir 'thoth.exe') -Force
  Write-Host "==> Installed to $InstallDir\thoth.exe"

  # Add the install dir to the user PATH if it isn't already there.
  $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
  if (($userPath -split ';') -notcontains $InstallDir) {
    [Environment]::SetEnvironmentVariable('Path', "$userPath;$InstallDir", 'User')
    Write-Host "==> Added $InstallDir to your PATH (restart the terminal to use 'thoth')."
  }
  Write-Host "Done. Thoth $tag installed." -ForegroundColor Green
}
finally {
  Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
