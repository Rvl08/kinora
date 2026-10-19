# Security policy

Kinora is pre-release software. Until a public release exists, report vulnerabilities privately to the maintainers
through GitHub's private vulnerability reporting on this repository. Please do not open public issues for security problems.

Areas of particular interest:

- The local API and MCP server (loopback binding, token authentication)
- Model and update downloads (SHA-256 and signature verification)
- Parsing of untrusted media, project files, templates and effect packages
