# scripts/sandbox

> **Status, 24 Oct 2026:** Smart App Control is now **enforced inside Windows Sandbox** too
> (`VerifiedAndReputablePolicyState = 1`, previously 2). Unsigned, unreputed binaries are blocked there: the pinned
> toolchain's `cargo.exe`, build scripts, test binaries, MSYS2's `bash.exe`. Windows builds and tests therefore run on
> GitHub Actions (`windows-latest`) instead. The runner below still works for signed or reputable tools only. Never try
> to disable or work around Smart App Control, inside the sandbox included.

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

Jobs:

- [`jobs/rust-check.ps1`](jobs/rust-check.ps1) runs fmt, clippy and tests, as CI does.
- [`jobs/ipc-contract.ps1`](jobs/ipc-contract.ps1) regenerates the TS bindings and the `project.json` schema,
  then runs fmt, clippy, tests and `cargo deny` (advisories use the host's RustSec database offline).
