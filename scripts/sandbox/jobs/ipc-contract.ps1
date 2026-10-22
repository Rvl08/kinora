# Sandbox job: regenerate the IPC contract files (TypeScript bindings, project.json schema),
# then run the workspace checks as CI does, plus cargo-deny when it is installed.
# Usage: powershell -ExecutionPolicy Bypass -File scripts/sandbox/invoke.ps1 -Name ipc-contract -Script scripts/sandbox/jobs/ipc-contract.ps1
$ErrorActionPreference = 'Stop'
$failed = 0
$steps = @(
  'cargo run -p kinora-ipc --example generate --locked',
  'cargo fmt --all --check',
  'cargo clippy --workspace --all-targets --locked -- -D warnings',
  'cargo test --workspace --locked'
)
if (Get-Command cargo-deny -ErrorAction SilentlyContinue) {
  $steps += 'cargo deny check licenses bans sources'
  # The sandbox has no git to fetch the RustSec database; use the host's copy (C:\host\cargo).
  $steps += 'cargo deny --offline check advisories'
} else {
  Write-Host 'cargo-deny not installed; skipping licence and advisory checks'
}
foreach ($step in $steps) {
  Write-Host "== $step"
  cmd /c "$step 2>&1"
  if ($LASTEXITCODE -ne 0) { Write-Host "FAILED: $step"; $failed++ }
}
exit $failed
