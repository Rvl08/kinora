# installer

Windows installer configuration (NSIS through the Tauri bundler): wizard languages (Indonesian, English),
licence page, per-user install without administrator rights, WebView2 embedded bootstrapper, `.kinora` file
association, and an uninstaller that can optionally remove downloaded models and caches.

Validated by Phase 0 Spike S4 ([docs/04-phase0-plan.md](../docs/04-phase0-plan.md)).

## Code signing

Every exe and dll in the bundle is Authenticode-signed ([ADR-0007](../docs/adr/0007-code-signing.md)):
Smart App Control blocks any unsigned DLL it is asked to load. The Tauri bundler signs the app, the
NSIS installer and the uninstaller through `scripts/signing/sign.mjs`:

```jsonc
// apps/desktop/src-tauri/tauri.conf.json
"bundle": { "windows": { "signCommand": { "cmd": "node", "args": ["../../../scripts/signing/sign.mjs", "%1"] } } }
```

The key is selected by environment (`KINORA_SIGN_MODE=store` + `KINORA_SIGN_THUMBPRINT` for a certificate
in the Windows store, such as a smartcard or Certum SimplySign). Bundled DLLs (FFmpeg, ggml backends) are
signed by the release job with `pnpm run sign <dir>`, and the release fails unless
`pnpm run sign:verify <dir>` passes. Signatures by Microsoft and Cisco on their own binaries are kept as is.
