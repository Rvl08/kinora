# Writes a Windows Sandbox definition that runs the signing self-test
# (selftest.ps1) against a throwaway, VM-only test certificate.
#
# Usage (host): powershell -ExecutionPolicy Bypass -File scripts/signing/sandbox/prepare.ps1
# Then:         start scripts/signing/sandbox/generated/signing-selftest.wsb
# Output:       tests/media/signing-out/selftest.json, selftest.log, verify-signed.json

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path "$PSScriptRoot\..\..\..").Path
$gen = Join-Path $PSScriptRoot 'generated'
New-Item -ItemType Directory -Force $gen | Out-Null
$out = Join-Path $repo 'tests\media\signing-out'
New-Item -ItemType Directory -Force $out | Out-Null
Remove-Item -ErrorAction SilentlyContinue (Join-Path $out 'selftest.json')

# Payload sources: the S3 spike's build cache (an exe built with the same toolchain) and vendor/.
$targetDir = Join-Path (Split-Path $repo) 'target-s3'
foreach ($need in "$targetDir\release\s3-ai.exe", "$targetDir\release\DirectML.dll", "$repo\vendor\ffmpeg-lgpl\bin\ffprobe.exe", "$repo\vendor\openh264\openh264-2.6.0-win64.dll") {
  if (-not (Test-Path $need)) { throw "missing $need (build Spike S3 / run pnpm run ffmpeg:fetch first)" }
}
$node = Split-Path (Get-Command node).Source

function Map($hostPath, $sandboxPath, $readOnly) {
  "    <MappedFolder><HostFolder>$hostPath</HostFolder><SandboxFolder>$sandboxPath</SandboxFolder><ReadOnly>$($readOnly.ToString().ToLower())</ReadOnly></MappedFolder>"
}
$maps = @(
  (Map $repo 'C:\k' $false),
  (Map $targetDir 'C:\t' $true),
  (Map $node 'C:\Program Files\nodejs' $true),
  (Map "${env:ProgramFiles(x86)}\Windows Kits\10" "${env:ProgramFiles(x86)}\Windows Kits\10" $true)
)
@"
<Configuration>
  <VGpu>Disable</VGpu>
  <Networking>Enable</Networking>
  <MemoryInMB>4096</MemoryInMB>
  <MappedFolders>
$($maps -join "`n")
  </MappedFolders>
  <LogonCommand>
    <Command>powershell -ExecutionPolicy Bypass -File C:\k\scripts\signing\sandbox\selftest.ps1</Command>
  </LogonCommand>
</Configuration>
"@ | Set-Content -Encoding utf8 (Join-Path $gen 'signing-selftest.wsb')
Write-Host "Prepared: $gen\signing-selftest.wsb"
