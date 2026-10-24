# Sandbox job (P0-05b): builds the static release FFmpeg and runs the release
# gate (build.mjs), then links, runs and inspects the Rust probe (probe.ps1),
# the same two steps as .github/workflows/ffmpeg-release.yml. Smart App
# Control on the dev machine blocks the freshly built configure probes and
# executables, hence the sandbox.
#
# Usage (host):
#   powershell -ExecutionPolicy Bypass -File scripts/sandbox/invoke.ps1 -Name ffmpeg-release `
#     -Script scripts/ffmpeg/release/sandbox-job.ps1 -TimeoutMinutes 180
#
# Output in tests/sandbox-out/<job>/: job.log, build.log, probe.log, dist/ (zip, sha256, sources/,
# verify-release.json, probe/), actionlint.txt. The artifact itself lands in
# vendor/ffmpeg-static/. Downloads and unpacked tools persist in
# C:\cache\ffmpeg-release between jobs.
#
# Optional: C:\cache\actionlint\actionlint_1.7.12_windows_amd64.zip (put there
# from the host) lets the job lint the workflows too.
$ErrorActionPreference = 'Stop'
$out = $env:JOB_OUT
$work = 'C:\kf'
$failed = 0

# Tee in UTF-8 (Tee-Object writes UTF-16 in Windows PowerShell 5.1).
function Save-Lines([string]$Path) {
  begin { Set-Content -Encoding utf8 $Path '' }
  process { $_; Add-Content -Encoding utf8 $Path $_ }
}

Write-Host '== build + release gate'
# Through cmd so stderr arrives as plain lines; also kept in build.log.
cmd /c "node scripts/ffmpeg/release/build.mjs --work $work --cache C:\cache\ffmpeg-release --tools C:\kft --out C:\k\vendor\ffmpeg-static --dist $out\dist 2>&1" |
  Save-Lines "$out\build.log"
if ($LASTEXITCODE -ne 0) { Write-Host "FAILED: build or release gate (exit $LASTEXITCODE)"; exit 1 }

cmd /c "powershell -ExecutionPolicy Bypass -File scripts\ffmpeg\release\probe.ps1 -Out $out\dist\probe -Target $work\target 2>&1" |
  Save-Lines "$out\probe.log"
if ($LASTEXITCODE -ne 0) { Write-Host "FAILED: probe (exit $LASTEXITCODE)"; $failed++ }

$lint = 'C:\cache\actionlint\actionlint_1.7.12_windows_amd64.zip'
if (Test-Path $lint) {
  Write-Host '== actionlint'
  $hash = (Get-FileHash -Algorithm SHA256 $lint).Hash.ToLower()
  if ($hash -ne '6e7241b51e6817ea6a047693d8e6fed13b31819c9a0dd6c5a726e1592d22f6e9') {
    Write-Host "FAILED: actionlint archive digest $hash"; $failed++
  } else {
    $dir = "$work\actionlint"
    Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $dir | Out-Null
    & "$env:SystemRoot\System32\tar.exe" -xf $lint -C $dir
    $result = cmd /c "$dir\actionlint.exe -no-color .github/workflows/ffmpeg-release.yml .github/workflows/ci.yml 2>&1"
    $code = $LASTEXITCODE
    $result | Set-Content -Encoding utf8 "$out\actionlint.txt"
    Write-Host ($result -join "`n")
    Write-Host "actionlint exit $code"
    if ($code -ne 0) { $failed++ }
  }
}
exit $failed
