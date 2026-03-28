# mchact

**mchact** is a fork of [mchact](https://github.com/mchact/mchact) — a Rust-based multi-platform agentic chat runtime. This fork focuses on building and integrating three companion projects:

- **mchact** — the core agentic chat engine (this codebase)
- **hermes-agent** — autonomous agent orchestration layer
- **clawteam** — multi-agent collaboration framework

## What is mchact?

mchact is a channel-agnostic AI chat bot written in Rust. It connects to multiple chat platforms through a single agentic loop, executes tools, manages sessions, and provides persistent memory — all from one binary.

### Supported Channels

| Channel | Status |
|---------|--------|
| Web UI | Built-in (React + Vite) |
| Telegram | Stable (teloxide) |
| Discord | Stable (serenity) |
| Slack | Supported |
| Feishu / Lark | Supported |
| Matrix | Supported |
| WhatsApp | Supported |
| WeChat | Supported |
| Signal | Supported |
| IRC | Supported |
| Nostr | Supported |
| DingTalk | Supported |
| QQ | Supported |
| iMessage | Supported |
| Email | Supported |

### LLM Providers

Provider-agnostic runtime supporting Anthropic, OpenAI, OpenRouter, Ollama, DeepSeek, Google, and any OpenAI-compatible API.

### Key Features

- **Agentic tool loop** — multi-step tool execution with session resume and context compaction
- **22+ built-in tools** — bash, file ops, web search/fetch, browser, memory, scheduling, MCP, A2A, and more
- **Persistent memory** — file-based (AGENTS.md) + structured SQLite memory with reflector and deduplication
- **Scheduled tasks** — cron-based background scheduler with override prompts
- **Sub-agents** — spawn isolated agent loops with restricted tool access
- **Skills & ClawHub** — skill discovery, activation, and registry integration
- **MCP support** — Model Context Protocol server/tool federation
- **A2A protocol** — agent-to-agent HTTP integration for multi-agent routing
- **Hooks** — extensible event system (BeforeLLMCall, BeforeToolCall, AfterToolCall)
- **Plugins** — plugin runtime with manifest-based discovery
- **Observability** — OpenTelemetry metrics/traces/logs, Langfuse, AgentOps adapters
- **Web UI** — built-in React interface with auth, sessions, streaming, config panel
- **Soul system** — customizable personality via SOUL.md with per-chat overrides
- **Sandbox** — configurable security sandbox for tool execution
- **Voice** — speech-to-text via OpenAI Whisper or local transcription

## Project Focus

This repository (**mchact**) serves as the integration workspace where mchact, hermes-agent, and clawteam converge:

### mchact (core engine)
The upstream agentic chat runtime. Handles LLM communication, tool execution, session management, and channel adapters. This is the foundation everything else builds on.

### hermes-agent (orchestration)
Autonomous agent orchestration layer. Hermes manages agent lifecycles, task delegation, and inter-agent communication. It builds on mchact's sub-agent and A2A capabilities to coordinate complex multi-step workflows.

### clawteam (collaboration)
Multi-agent collaboration framework. ClawTeam enables teams of specialized agents to work together — planning, executing, reviewing, and iterating — as a coordinated unit. It leverages hermes-agent for orchestration and mchact for execution.

## Architecture

```
crates/
  mchact-core/        Shared types, errors, text utilities
  mchact-storage/     SQLite persistence, memory domain, usage reports
  mchact-tools/       Tool runtime primitives, sandbox, path guards
  mchact-channels/    Channel abstraction and delivery boundary
  mchact-clawhub/     ClawHub registry client, install, lockfile
  mchact-app/         App-level support (logging, skills, transcribe)
  mchact-observability/ OTLP metrics/traces/logs export

src/
  main.rs                CLI entry point (start, setup, doctor, help)
  runtime.rs             App wiring, provider/tool init, channel boot
  agent_engine.rs        Core agentic loop (process_with_agent)
  llm.rs                 Provider implementations + format translation
  web.rs                 Web API routes and streaming
  scheduler.rs           Background task scheduler + memory reflector
  channels/*.rs          Platform adapters (16 channels)
  tools/*.rs             Built-in tool implementations (22+ tools)

web/                     React + Vite web UI (embedded into binary)
```

## Quick Start

```sh
# Build
cargo build

# Interactive setup wizard
cargo run -- setup

# Start the bot
cargo run -- start

# Diagnostics
cargo run -- doctor
```

### Configuration

Copy `mchact.config.example.yaml` to `mchact.config.yaml` and configure:
- LLM provider and API key
- Channels to enable (web, telegram, discord, etc.)
- Data directory and working directory
- Optional: observability, sandbox, voice, plugins

See [README-origin.md](README-origin.md) for the full original documentation.

## License

MIT
