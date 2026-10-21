# Privacy policy

Kinora is an offline desktop video editor. It has no account, no telemetry, no analytics and no crash
reporting, and it never uploads your media, projects or recordings.

Kinora is pre-release software. This policy describes how it is designed to behave; any change to it is made in
this file, in the same repository as the code, before the behaviour changes.

## What stays on your computer

Everything you import, edit, transcribe, generate or export. AI features (captions, background removal,
text-to-speech, noise reduction and others) run on your own CPU or GPU.

## When Kinora connects to the internet

Only to download something you asked for, from the fixed addresses below. Each download is verified against a
SHA-256 digest pinned in this repository before it is used.

| What | When | Where it comes from |
|---|---|---|
| AI models | The first time you use a feature that needs one | Pinned URLs in [`models/manifest.toml`](models/manifest.toml) (Hugging Face, GitHub) |
| OpenH264 (H.264 encoder) | The first time you export H.264 without a hardware encoder, after you accept Cisco's licence | Cisco (`ciscobinary.openh264.org`) |
| DirectML runtime | The first time an AI feature runs on the GPU | Microsoft's NuGet package |
| Update check | When you check for updates; automatic checks can be turned off | This project's GitHub Releases |

These requests carry no identifier of you or your device beyond what any HTTPS request carries (your IP address
and a user agent naming Kinora and its version), and Kinora keeps no log of them outside your computer.

## Optional features that send data elsewhere

- **AI Copilot with your own API key** (off by default): when you configure a cloud model provider and use the
  Copilot, your prompt and the parts of your project it needs are sent to **that provider**, under their privacy
  policy. With a local model (for example Ollama) nothing leaves your computer.

Kinora shows which of these is active in its settings, and each can be turned off.

## Third-party components

ONNX Runtime is built into Kinora with its telemetry switched off. Microsoft Store installations are also subject to the
Microsoft Store's own terms.

## Contact

Questions about this policy: open an issue in this repository. Security problems: see [SECURITY.md](SECURITY.md).
