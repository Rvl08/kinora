# Runs inside Windows Sandbox (LogonCommand written by invoke.ps1): prepares the
# toolchain, runs one job script, records its exit code, shuts the VM down.

param([Parameter(Mandatory)][string]$Name, [Parameter(Mandatory)][string]$Script)

# The logon command starts unelevated; jobs may need the machine (installers, certificate stores).
$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) {
  Start-Process powershell -Verb RunAs -Wait -ArgumentList '-ExecutionPolicy', 'Bypass', '-File', $PSCommandPath, '-Name', $Name, '-Script', "`"$Script`""
  exit
}

$env:JOB_OUT = "C:\k\tests\sandbox-out\$Name"
New-Item -ItemType Directory -Force $env:JOB_OUT | Out-Null
Start-Transcript -Path "$env:JOB_OUT\job.log" | Out-Null
$exit = 1
try {
  . 'C:\k\scripts\sandbox\generated\env.ps1'

  # A clean sandbox has no VC++ runtime; programs built with the host's MSVC need it.
  $redist = Get-ChildItem "$env:VSINSTALLDIR\VC\Redist\MSVC\*\vc_redist.x64.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($redist) { Start-Process $redist.FullName -ArgumentList '/install', '/quiet', '/norestart' -Wait }

  # Executables cannot run from a read-write mapped folder (error 1392): build on the VM's disk,
  # carrying the cache in C:\t between sessions.
  robocopy C:\t C:\b /E /MT:16 /NFL /NDL /NJH /NJS /NP | Out-Null
  $env:CARGO_TARGET_DIR = 'C:\b'

  Set-Location C:\k
  Write-Host "job $Name`: $Script ($(Get-Date -Format o))"
  # Through cmd so stdout and stderr arrive as plain lines, and through Out-Host so the transcript records them.
  cmd /c "powershell -NoProfile -ExecutionPolicy Bypass -File `"$(Join-Path C:\k $Script)`" 2>&1" | Out-Host
  $exit = $LASTEXITCODE

  robocopy C:\b C:\t /MIR /MT:16 /NFL /NDL /NJH /NJS /NP | Out-Null
} catch {
  Write-Host "guest error: $($_.Exception.Message)"
} finally {
  [ordered]@{ name = $Name; script = $Script; exitCode = $exit; finished = (Get-Date).ToString('o') } |
    ConvertTo-Json | Set-Content -Encoding utf8 "$env:JOB_OUT\done.json"
  Stop-Transcript | Out-Null
  shutdown /s /t 5
}
