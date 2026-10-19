# effects

One folder per effect package. A package is data, not engine code:

- `effect.toml`: id, kind (look, adjust, effect, transition, audio, text), parameters with ranges and defaults
- `effect.wgsl`: the shader, shared by the WebGPU preview and the wgpu exporter
- `fixtures.toml`: reference inputs and expected outputs for golden-frame tests

Every package is written from scratch for Kinora. Do not copy or port effects, presets or shaders from other
editors (see [ADR-0002](../docs/adr/0002-license-and-clean-room.md)).
