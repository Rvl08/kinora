# Sandbox job: the workspace's Rust checks, as CI runs them.
# Usage: powershell -ExecutionPolicy Bypass -File scripts/sandbox/invoke.ps1 -Name rust-check -Script scripts/sandbox/jobs/rust-check.ps1
$ErrorActionPreference = 'Stop'
$failed = 0
foreach ($step in @('cargo fmt --all --check', 'cargo clippy --workspace --all-targets --locked -- -D warnings', 'cargo test --workspace --locked')) {
  Write-Host "== $step"
  cmd /c "$step 2>&1"
  if ($LASTEXITCODE -ne 0) { Write-Host "FAILED: $step"; $failed++ }
}
exit $failed
