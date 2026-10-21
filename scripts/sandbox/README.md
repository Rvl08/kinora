# scripts/sandbox

Runs a job script inside Windows Sandbox and waits for it. The dev machine enforces Smart App Control, which
blocks freshly built unsigned executables (Cargo build scripts and test binaries, FFmpeg's configure probes),
so anything that builds and runs native code runs here.

```powershell
powershell -ExecutionPolicy Bypass -File scripts/sandbox/invoke.ps1 -Name rust-check -Script scripts/sandbox/jobs/rust-check.ps1
```

- One sandbox per machine: concurrent invocations queue on a lock file next to the repository.
- Inside the sandbox:
  - Paths: the repository is at `C:\k` (read-write), and `C:\cache` holds persistent downloads.
  - Tools on `PATH`: MSVC, the Windows SDK, Rust, CMake, LLVM, Node, Ninja, and the Vulkan SDK when installed.
  - The VC++ runtime is installed, and `CARGO_TARGET_DIR` is the VM's own disk, cached between jobs.
  - Jobs run elevated, and `$env:JOB_OUT` is `tests/sandbox-out/<name>/`.
- The exit code is the job's. The log is in `tests/sandbox-out/<name>/job.log`.
- Each job costs about 4 minutes of VM start-up, so batch related work into one job.

Jobs: [`jobs/rust-check.ps1`](jobs/rust-check.ps1) runs fmt, clippy and tests, as CI does.
