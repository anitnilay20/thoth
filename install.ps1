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

  # Install the bundled wasm plugins next to the exe — the app loads them from
  # `<exe dir>\assets\plugins`. Older archives without them just skip this.
  $pluginsSrc = Join-Path $exe.DirectoryName 'assets\plugins'
  if (Test-Path $pluginsSrc) {
    $pluginsDest = Join-Path $InstallDir 'assets\plugins'
    $pluginsTmp  = Join-Path $InstallDir 'assets\plugins.tmp'
    New-Item -ItemType Directory -Force -Path (Join-Path $InstallDir 'assets') | Out-Null
    # Copy to a staging dir, then swap — so a failed copy can't leave a partial
    # plugins dir in place of a working one.
    if (Test-Path $pluginsTmp) { Remove-Item -Recurse -Force $pluginsTmp }
    try {
      Copy-Item -Recurse -Force $pluginsSrc $pluginsTmp
      if (Test-Path $pluginsDest) { Remove-Item -Recurse -Force $pluginsDest }
      Move-Item $pluginsTmp $pluginsDest
      Write-Host "==> Installed bundled plugins to $InstallDir\assets\plugins"
    } catch {
      if (Test-Path $pluginsTmp) { Remove-Item -Recurse -Force $pluginsTmp }
      throw
    }
  }

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
