# mchact

mchact is a Rust multi-channel agent runtime that exposes one tool-using chat engine across local web, Telegram, Discord, Slack, Feishu/Lark, Matrix, WhatsApp, Weixin, Signal, IRC, Nostr, email, and more. The core design keeps the agent loop, tool system, memory pipeline, scheduler, MCP federation, and observability stack channel-agnostic, so operators can run the same assistant logic in different transports without re-implementing the runtime.

## Tech Stack

| Area | Main libraries / tools | Version(s) from repo |
|---|---|---|
| Language and toolchain | Rust edition 2021, `rust-toolchain.toml` | Rust `1.93.1` |
| CLI and runtime | `clap`, `tokio`, `anyhow`, `tracing` | `clap 4.6`, `tokio 1`, `tracing 0.1` |
| LLM transport | native Anthropic + OpenAI-compatible providers, `reqwest` | `reqwest 0.12` |
| Channels | `teloxide`, `serenity`, `matrix-sdk`, Axum web adapter | `0.17`, `0.12`, `0.16.0`, `axum 0.7` |
| Storage | `rusqlite` with bundled SQLite, optional Postgres feature, object storage backends | `rusqlite 0.37` |
| Tool and protocol integrations | `rmcp`, `agent-client-protocol` | `rmcp 1.3`, `agent-client-protocol 0.10.3` |
| Auth and security | `argon2`, sandbox + path guards in `mchact-tools` | `argon2 0.5` |
| Observability | OpenTelemetry OTLP exporters + vendor adapters | `opentelemetry-proto 0.28` |
| Web UI | React, Vite, TypeScript, Tailwind v4, Radix Themes | React `18.3.1`, Vite `5.4.10`, TypeScript `5.6.3`, Tailwind `4.2.1`, `@radix-ui/themes 3.2.1` |

## Install and Run

### Prerequisites

- Rust `1.93.1` with `cargo`
- Node.js `20+` for the embedded web UI build
- A valid `mchact.config.yaml`

### Local development

```sh
cp mchact.config.example.yaml mchact.config.yaml
cargo build
npm --prefix web ci
npm --prefix web run build
cargo run -- setup
cargo run -- start
```

### Docker

```sh
cp mchact.config.example.yaml mchact.config.yaml
docker compose up --build
```

The default local web UI listens on `127.0.0.1:10961` when `channels.web.enabled: true`.

## Environment Variables

mchact is config-first. There is no `.env.example`; most operator settings live in [`mchact.config.example.yaml`](/Volumes/Data/Codes/Local/mchact/mchact.config.example.yaml). These environment variables are read directly by the codebase:

### Core runtime

| Variable | Purpose |
|---|---|
| `MCHACT_CONFIG` | Override config file path instead of `./mchact.config.yaml` / `.yml`. |
| `MCHACT_SKILLS_DIR` | Override the resolved skills directory. |
| `RUST_LOG` | Standard Rust log filter for CLI, gateway, and web runtime logging. |
| `MCHACT_PATH_ALLOWLIST` | Extends the file/path guard allowlist used by file tools. |
| `MCHACT_SANDBOX_MOUNT_ALLOWLIST` | Adds extra host paths that sandboxed tools may mount. |

### Provider and auth tokens

| Variable | Purpose |
|---|---|
| `OPENAI_CODEX_ACCESS_TOKEN` | OAuth-style token for the `openai-codex` provider path. |
| `CODEX_HOME` | Alternate home directory for Codex auth/runtime files. |
| `QWEN_PORTAL_ACCESS_TOKEN` | Token used by the Qwen auth path. |
| `QWEN_CODE_ACCESS_TOKEN` | Code access token for Qwen-backed flows. |
| `QWEN_HOME` | Alternate home directory for Qwen auth/runtime files. |

### Storage backend fallbacks

| Variable | Purpose |
|---|---|
| `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` | S3-compatible object storage credentials when not embedded in config. |
| `AZURE_STORAGE_CONNECTION_STRING` / `AZURE_STORAGE_KEY` | Azure Blob auth fallback. |
| `GOOGLE_APPLICATION_CREDENTIALS` | GCS credentials path fallback. |

### Gateway, automation, and training

| Variable | Purpose |
|---|---|
| `MCHACT_GATEWAY_TOKEN` | Gateway RPC / compatibility bridge bearer token. |
| `OPENCLAW_GATEWAY_TOKEN` | Alternate accepted gateway token name. |
| `GATEWAY_TOKEN` | Alternate accepted gateway token name. |
| `MCHACT_API_KEY` | Alternate API key name accepted by gateway bridge auth. |
| `TINKER_API_KEY` | Training / RL integration token. |
| `WANDB_API_KEY` | Weights & Biases auth for training pipelines. |
| `WANDB_ENTITY` | Weights & Biases entity override. |

> ⚠️ Needs clarification: provider-specific API keys for Anthropic, OpenAI-compatible endpoints, Telegram, Discord, Slack, etc. are configured primarily through `mchact.config.yaml`, not through a repo-wide env contract.

## Available Scripts

### Core commands

- `cargo run -- setup`: launch the interactive setup wizard.
- `cargo run -- start`: start the runtime with all enabled channels.
- `cargo run -- doctor`: run environment and config diagnostics.
- `cargo build`: compile the workspace.
- `cargo test`: run Rust tests.

### Web UI

- `npm --prefix web ci`: install frontend dependencies.
- `npm --prefix web run dev`: start the Vite dev server for `web/`.
- `npm --prefix web run build`: build the embedded SPA in `web/dist`.

### Docs and validation

- `node scripts/generate_docs_artifacts.mjs`: regenerate generated docs from source.
- `node scripts/generate_docs_artifacts.mjs --check`: fail if generated docs are stale.
- `./check.sh`: quick local validation used by contributors.
- `scripts/ci/stability_smoke.sh`: focused stability regression suite.
- `scripts/ci/nightly_stability.sh`: broader nightly stability run.

### Helper scripts

- `./start.sh`: build the web UI and start the runtime with a local config path.
- `./install.sh` / `install.ps1`: install the binary and assets locally.
- `scripts/test_http_hooks.sh`: smoke-test HTTP hook endpoints.
- `scripts/matrix-smoke-test.sh`: matrix/provider smoke test harness.

## Folder Structure

```text
.
├── src/                      # Main binary crate: CLI, runtime wiring, agent loop, channels, tools, web, scheduler
│   ├── channels/             # Channel adapters and channel-specific runtime setup
│   ├── clawhub/              # ClawHub CLI/service/tool integration
│   ├── parsers/              # Training/export parser adapters
│   ├── tools/                # Built-in tool implementations and tool registry assembly
│   └── web/                  # Axum route modules, auth middleware, SSE/WS/session APIs
├── crates/                   # Reusable workspace crates split by domain boundary
│   ├── mchact-core/          # Shared errors, text utilities, and LLM/tool data types
│   ├── mchact-storage/       # DB traits, schema, migrations, SQLite/Postgres-backed persistence
│   ├── mchact-storage-backend/ # Object storage abstraction for local/cloud file payloads
│   ├── mchact-tools/         # Tool runtime primitives, path guards, sandbox, auth context
│   ├── mchact-channels/      # Channel interfaces and delivery abstractions
│   ├── mchact-app/           # App support code such as logging and built-in skills
│   ├── mchact-memory/        # Observation store, retrieval, injection, and derivation pipeline
│   ├── mchact-media/         # STT/TTS/image/video/document media provider routing
│   ├── mchact-observability/ # OTLP metrics/traces/logs exporters and vendor adapters
│   └── mchact-clawhub/       # ClawHub registry client, install logic, and lockfile handling
├── web/                      # React SPA bundled into the Rust binary via `include_dir`
├── docs/                     # Operator docs, RFCs, runbooks, generated references, and reports
├── hooks/                    # Sample runtime hooks with `HOOK.md` manifests
├── skills/                   # Built-in skill assets shipped with the runtime
├── scripts/                  # Docs generation, CI helpers, installers, and smoke tests
├── tests/                    # Integration and regression tests
├── training/                 # Training environment assets and datasets
├── examples/                 # Example plugin and integration assets
├── packaging/                # Packaging helpers such as Windows installer assets
├── shared/                   # Shared temp/runtime helper assets used by tooling
├── screenshots/              # UI screenshots used in docs and release materials
├── docker-compose.yaml       # Local container orchestration entrypoint
├── Dockerfile                # Multi-stage image that embeds the web UI into the binary
└── mchact.config.example.yaml # Canonical config template for operators
```
