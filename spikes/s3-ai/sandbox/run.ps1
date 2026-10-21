# Runs inside Windows Sandbox (LogonCommand of generated/kinora-s3.wsb).
# Builds Spike S3 with the host's mapped toolchains, runs the tasks listed in
# tests/media/s3-out/sandbox-task.txt, writes logs next to it, then shuts the
# sandbox down so the next session can start.
#
# Executables cannot run straight from a mapped host folder ("The file or
# directory is corrupted and unreadable", os error 1392), so Cargo builds on
# the sandbox's own disk (C:\b). The mapped C:\t only carries the build cache
# between sessions: restored at the start, saved at the end.

$out = 'C:\k\tests\media\s3-out'
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
Start-Transcript -Path "$out\sandbox-$stamp.log" | Out-Null
$status = [ordered]@{ started = (Get-Date).ToString('o') }

# Environment facts the report needs: is Smart App Control present here, which GPU is visible.
$ci = Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\CI\Policy' -ErrorAction SilentlyContinue
$status.smartAppControl = if ($ci) { $ci.VerifiedAndReputablePolicyState } else { 'absent' }
$status.gpus = @(Get-CimInstance Win32_VideoController | ForEach-Object { $_.Name })
$status.vulkanLoader = Test-Path "$env:SystemRoot\System32\vulkan-1.dll"
Write-Host ($status | ConvertTo-Json)

. 'C:\k\spikes\s3-ai\sandbox\generated\env.ps1'

# The clean sandbox image carries an older (or no) VC++ runtime. Programs built
# here with the host's newer MSVC STL crash against it (0xC0000005 in
# std::mutex, e.g. whisper.cpp's vulkan-shaders-gen), so install the runtime
# that matches the toolset first.
$crt = "$env:SystemRoot\System32\msvcp140.dll"
$status.msvcp140Before = if (Test-Path $crt) { (Get-Item $crt).VersionInfo.FileVersion } else { 'absent' }
$redist = Get-ChildItem "$env:VSINSTALLDIR\VC\Redist\MSVC\*\vc_redist.x64.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
if ($redist) {
  Start-Process $redist.FullName -ArgumentList '/install', '/quiet', '/norestart' -Wait
  $status.msvcp140After = (Get-Item $crt).VersionInfo.FileVersion
}
Set-Location 'C:\k\spikes\s3-ai'

$t0 = Get-Date
robocopy C:\t C:\b /E /MT:16 /NFL /NDL /NJH /NJS /NP | Out-Null
$status.cacheRestoreS = [int]((Get-Date) - $t0).TotalSeconds
$env:CARGO_TARGET_DIR = 'C:\b'

$t0 = Get-Date
$features = (Get-Content "$out\sandbox-features.txt" -Raw -ErrorAction SilentlyContinue)
$features = if ($features) { $features.Trim() } else { '' }
$status.features = $features
$featureArg = if ($features) { "--features $features" } else { '' }
if ($env:VULKAN_SDK) { $env:Path = "$env:VULKAN_SDK\Bin;" + $env:Path }
cmd /c "cargo build --release $featureArg > `"$out\build.log`" 2>&1"
$status.buildExit = $LASTEXITCODE
if ($status.buildExit -ne 0 -and $features) {
  # Keep the session useful: fall back to the default (CPU) build and say so.
  $status.featureBuildExit = $status.buildExit
  Move-Item -Force "$out\build.log" "$out\build-features.log"
  cmd /c "cargo build --release > `"$out\build.log`" 2>&1"
  $status.buildExit = $LASTEXITCODE
  $status.features = ''
}
$status.buildS = [int]((Get-Date) - $t0).TotalSeconds
if ($status.buildExit -eq 0) {
  $tasks = (Get-Content "$out\sandbox-task.txt" -Raw).Trim() -split ';' | Where-Object { $_ }
  foreach ($t in $tasks) {
    # "bench asr,tts" -> task-bench_asr_tts.log
    $name = ($t.Trim() -replace '[^A-Za-z0-9]+', '_')
    $t0 = Get-Date
    cmd /c "C:\b\release\s3-ai.exe $($t.Trim()) > `"$out\task-$name.log`" 2>&1"
    $status["task_$name"] = $LASTEXITCODE
    $status["task_${name}_s"] = [int]((Get-Date) - $t0).TotalSeconds
  }
}

$t0 = Get-Date
robocopy C:\b C:\t /MIR /MT:16 /NFL /NDL /NJH /NJS /NP | Out-Null
$status.cacheSaveS = [int]((Get-Date) - $t0).TotalSeconds
$status.finished = (Get-Date).ToString('o')
$status | ConvertTo-Json | Set-Content -Encoding utf8 "$out\sandbox.done"
Stop-Transcript | Out-Null
shutdown /s /t 10
