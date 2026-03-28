# AGENTS.md

## Project overview

mchact is a Rust multi-channel agent runtime with one shared agent loop and one provider-agnostic LLM layer. The same core runtime serves the local web UI/API plus chat adapters including Telegram, Discord, Slack, Feishu/Lark, Matrix, WhatsApp, Weixin, Signal, IRC, Nostr, QQ, DingTalk, iMessage, and email.

Core capabilities:
- Tool-using chat agent loop with multi-step tool execution
- Session resume, compaction, and persisted per-session settings
- File memory plus structured memory with reflection and observability
- Scheduled tasks, DLQ replay, and background scheduler jobs
- Skills, plugins, MCP tool federation, and ClawHub skill install/search
- Web operator UI with auth, API keys, audit, metrics, and streaming
- A2A HTTP integration and ACP server / ACP-backed subagents
- Knowledge collections, media handling, batch/export/train/RL workflows

## Tech stack

- Language: Rust 2021
- Toolchain: Rust `1.93.1`
- CLI: `clap`
- Async runtime: `tokio`
- Web server/API: `axum`
- Web UI: React + Vite + TypeScript in `web/`
- Database: SQLite by default via `rusqlite`, optional Postgres feature
- Object storage: local disk, optional S3 / Azure / GCS backends
- LLM runtime: native Anthropic plus OpenAI-compatible providers
- MCP: `rmcp`
- ACP: `agent-client-protocol`
- Telegram: `teloxide`
- Discord: `serenity`
- Matrix: `matrix-sdk`

## Source index (`src/` + `crates/`)

Main orchestration files in `src/`:
- `main.rs`: CLI entry and command routing (`start`, `setup`, `doctor`, `gateway`, `skill`, `hooks`, `acp`, `knowledge`, `train`, `batch`, `export`, `rl`)
- `runtime.rs`: builds `AppState`, wires storage, providers, tools, hooks, channels, media, observability
- `agent_engine.rs`: shared agent loop, prompt assembly, memory injection, compaction, tool loop, persistence
- `config.rs`: config schema, defaults, normalization, compatibility migration, path resolution
- `llm.rs`: provider implementations, streaming, format translation, provider-specific compatibility logic
- `scheduler.rs`: scheduled tasks, reflector/background job loop
- `memory_backend.rs`: structured memory provider abstraction with local + MCP-backed fallback path
- `memory_service.rs`: memory extraction/injection logic and explicit remember flow
- `knowledge.rs`: knowledge collections, chunking, query helpers
- `knowledge_scheduler.rs`: embedding / observation / autogroup background processing
- `hooks.rs`: hook discovery, runtime execution, CLI management
- `skills.rs`: skill discovery, availability filtering, enable/disable handling
- `mcp.rs`: MCP server config, connection management, retries, circuit breaker, bulkhead, cache
- `plugins.rs`: manifest-driven plugin commands, tools, and context providers
- `gateway.rs`: gateway/service lifecycle and bridge RPC support
- `acp.rs`: ACP server mode over stdio
- `acp_subagent.rs`: ACP-backed subagent runtime
- `a2a.rs`: local A2A agent card and peer config
- `chat_commands.rs`: in-chat slash-style command handling and persisted overrides
- `web.rs`: Axum router, embedded SPA, auth bootstrap, SSE/WS/run hubs
- `web/*.rs`: web auth, middleware, sessions, config, metrics, MCP, stream, skills, ws helpers
- `channels/*.rs`: concrete adapters for Telegram, Discord, Slack, Feishu, Matrix, WhatsApp, Weixin, Signal, IRC, Nostr, QQ, DingTalk, iMessage, email
- `tools/*.rs`: built-in tool implementations and registry assembly
- `clawhub/*.rs`: runtime-side ClawHub CLI/service/tool wrappers
- `batch.rs`, `batch_worker.rs`, `export.rs`, `rl.rs`, `train_pipeline.rs`: offline generation/export/training flows

Workspace crates in `crates/`:
- `mchact-core`: shared errors, text helpers, LLM/tool data types
- `mchact-storage`: DB traits, schema, migrations, query helpers, SQLite/Postgres support
- `mchact-storage-backend`: object storage abstraction for local/cloud file payloads
- `mchact-tools`: tool runtime primitives, auth context, sandbox, path guards, web/todo helpers
- `mchact-channels`: channel abstractions and delivery/routing helpers
- `mchact-app`: logging, built-in skills, transcription support
- `mchact-memory`: observation store, derivation, injection, search
- `mchact-media`: STT/TTS/image/video/document provider routing
- `mchact-observability`: OTLP metrics/traces/logs exporters and adapters
- `mchact-clawhub`: ClawHub registry client, installer, lockfile logic

## Tool system

`src/tools/mod.rs` assembles the built-in registry, while shared runtime primitives live in `mchact-tools::runtime`:
- `Tool` trait (`name`, `definition`, `execute`)
- `ToolRegistry` dispatch, auth-context injection, working-dir resolution
- approval/risk gate for high-risk tools
- sandbox routing and path guard enforcement

Built-in tool docs are generated from code:
- `docs/generated/tools.md`

Regenerate docs artifacts with:
```sh
node scripts/generate_docs_artifacts.mjs
```

## Skills storage

- Default skills dir: `<data_dir>/skills`
- Config override: `skills_dir` in `mchact.config.yaml`
- Env override: `MCHACT_SKILLS_DIR`
- Built-in/runtime skills can also be backed by object storage
- ClawHub lockfile path: `<data_dir>/clawhub.lock.json`

## Agent loop (high level)

`process_with_agent` / `process_with_agent_with_events` flow:
1. Optional explicit-memory fast path handles direct remember commands
2. Load resumable session, or rebuild from persisted message history
3. Build system prompt from soul/file memory, structured memory, skills, plugins, and runtime context
4. Compact old context if session limits are exceeded
5. Call the configured provider with tool schemas
6. If the model returns tool calls, execute them through `ToolRegistry`
7. Append tool results and continue until `end_turn`
8. Persist session/message state and deliver or stream the final reply

## Memory architecture

Two primary layers:

1. File memory
- Global: runtime-scoped `AGENTS.md` file memory
- Chat-scoped memory files under the runtime data tree

2. Structured memory
- `memories` table with category, confidence, source, timestamps, archive/supersede lifecycle
- explicit remember fast path
- reflector extraction from conversation history
- dedup/supersede handling and injection logging

Related observability surfaces:
- `/api/usage`
- `/api/memory_observability`
- web usage/metrics views

## Database and storage

`mchact-storage` owns:
- schema creation and migrations
- chats, messages, sessions, session settings
- scheduled tasks, DLQ/history
- structured memory
- auth passwords, auth sessions, API keys
- audit logs
- metrics history
- knowledge metadata and access control

Object/file payloads are handled separately through `mchact-storage-backend`:
- memory files
- media/uploads/generated assets
- exports and archives
- runtime skill/state files

## Web/API

`web.rs` and `src/web/*` expose:
- send / send_stream and SSE replay
- auth APIs (`/api/auth/*`) with operator password, session cookie, API key scopes
- sessions/history/reset/delete/fork/tree
- config read/update and self-check
- metrics snapshot/summary/history
- usage report
- memory observability series
- A2A endpoints
- websocket bridge / Mission Control-compatible session methods

Auth model:
- operator password hash in DB
- `mc_session` and `mc_csrf` cookies for browser sessions
- scoped API keys: `operator.read`, `operator.write`, `operator.admin`, `operator.approvals`
- bootstrap token fallback if first-time password setup cannot be stored normally

## Hooks

Hook assets and spec:
- hook dirs: `hooks/<name>/HOOK.md`
- spec doc: `docs/hooks/HOOK.md`
- sample hooks: `hooks/block-bash/`, `hooks/block-global-memory/`, `hooks/filter-global-structured-memory/`, `hooks/redact-tool-output/`

Hook runtime supports:
- events: `BeforeLLMCall`, `BeforeToolCall`, `AfterToolCall`
- outcomes: `allow`, `block`, `modify`

## ClawHub

- doc: `docs/clawhub/overview.md`
- CLI: `mchact skill search|install|list|inspect|available`
- agent tools: `clawhub_search`, `clawhub_install` when `clawhub_agent_tools_enabled` is true

## Observability docs

- metrics docs: `docs/observability/metrics.md`
- observability architecture: `docs/observability/architecture.md`
- runbook: `docs/operations/runbook.md`
- release checklist: `docs/releases/pr-release-checklist.md`
- upgrade guide: `docs/releases/upgrade-guide.md`

OTLP-related queue/retry tuning includes:
- `otlp_queue_capacity`
- `otlp_retry_max_attempts`
- `otlp_retry_base_ms`
- `otlp_retry_max_ms`

## Build and test

```sh
cargo build
cargo test
npm --prefix web run build
```

Docs drift guard:
```sh
node scripts/generate_docs_artifacts.mjs --check
```

Repo-local validation shortcut:
```sh
zsh check.sh
```

## Collaboration conventions

- Treat the live source tree as authoritative; keep this file in sync when major modules, docs paths, or validation commands change.
- Do not assume a separate `website/` repo exists; the current frontend/docs surface in this repo is `web/` plus `docs/`.
