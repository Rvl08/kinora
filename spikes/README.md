# spikes

Throwaway Phase 0 prototypes (S1 preview, S2 media, S3 AI runtime, S4 installer).

Rules:

- Spike code is never shipped and never moved into `crates/` or `apps/` as is; production code is rewritten.
- Spikes are excluded from the Cargo workspace and the pnpm workspace.
- Each spike ends with a measured report in `docs/spikes/`.
