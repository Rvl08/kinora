# Proves that the static release FFmpeg links into a Rust executable and works
# (P0-05b): builds spikes/ffmpeg-static-probe against vendor/ffmpeg-static,
# runs it, checks with dumpbin that the executable imports no FFmpeg, codec or
# CRT DLL, and records the size FFmpeg adds. Used by the GitHub workflow and
# by the sandbox job, after scripts/ffmpeg/release/build.mjs.
#
# Usage: powershell -ExecutionPolicy Bypass -File scripts/ffmpeg/release/probe.ps1 -Out <dir> [-Target <dir>]
# Writes <Out>/probe.json, <Out>/dependents.txt and <Out>/sizes.json.
param(
  [Parameter(Mandatory)][string]$Out,
  # Executables cannot run from Windows Sandbox's mapped folders: keep it on a local disk.
  [string]$Target = 'C:\kf\target'
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path "$PSScriptRoot\..\..\..").Path
New-Item -ItemType Directory -Force $Out | Out-Null

function Find-Dumpbin {
  $onPath = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
  if ($onPath) { return $onPath.Source }
  $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
  $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
  $found = Get-ChildItem "$vs\VC\Tools\MSVC\*\bin\Hostx64\x64\dumpbin.exe" | Sort-Object FullName | Select-Object -Last 1
  if (-not $found) { throw 'dumpbin.exe not found' }
  return $found.FullName
}

Write-Host '== probe: cargo build --release'
$env:CARGO_TARGET_DIR = $Target
$t0 = Get-Date
Push-Location "$repo\spikes\ffmpeg-static-probe"
try {
  cmd /c 'cargo build --release --locked 2>&1'
  if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)" }
} finally { Pop-Location }
$buildSeconds = [int]((Get-Date) - $t0).TotalSeconds

$exe = Join-Path $Target 'release\ffmpeg-static-probe.exe'
$baseline = Join-Path $Target 'release\size-baseline.exe'

Write-Host '== probe: run'
& $exe (Join-Path $Target 'probe-media') --json (Join-Path $Out 'probe.json')
$probe = $LASTEXITCODE

Write-Host '== probe: dumpbin /dependents'
$dumpbin = Find-Dumpbin
$deps = (& $dumpbin /nologo /dependents $exe) -join "`n"
$deps | Set-Content -Encoding utf8 (Join-Path $Out 'dependents.txt')
Write-Host $deps
$dlls = @([regex]::Matches($deps, '(?im)^\s+(\S+\.dll)\s*$') | ForEach-Object { $_.Groups[1].Value.ToLower() })
# Static FFmpeg and static CRT: no FFmpeg or codec DLL, no VC++ runtime, no UCRT forwarders.
$forbidden = @($dlls | Where-Object {
    $_ -match '^(avcodec|avformat|avfilter|avutil|avdevice|swscale|swresample|postproc)' -or
    $_ -match '^(vcruntime|msvcp|ucrtbase|concrt|api-ms-win-crt-)' -or
    $_ -match '(dav1d|svtav1|vpx|opus|mp3lame|libvpl|zlib)'
  })
if ($forbidden.Count) { Write-Host "FAILED: unexpected imports: $($forbidden -join ', ')" }
else { Write-Host "imports OK: $($dlls -join ', ')" }

$sizes = [ordered]@{
  probe_exe_bytes    = (Get-Item $exe).Length
  baseline_exe_bytes = (Get-Item $baseline).Length
  ffmpeg_delta_bytes = (Get-Item $exe).Length - (Get-Item $baseline).Length
  imports            = $dlls
  cargo_build_seconds = $buildSeconds
}
$sizes | ConvertTo-Json | Set-Content -Encoding utf8 (Join-Path $Out 'sizes.json')
Write-Host ($sizes | ConvertTo-Json)

if ($probe -ne 0) { Write-Host "FAILED: probe exited $probe" }
exit ([int]($probe -ne 0) + [int]($forbidden.Count -gt 0))
