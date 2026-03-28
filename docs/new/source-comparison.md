# mchact Source Project Comparison

**Date:** 2026-03-27
**Purpose:** mchact is a synthesis of three projects. This document maps what came from where, what's been ported, and what remains.

---

## Project Overview

| Aspect | microclaw | hermes-agent | honcho |
|--------|-----------|-------------|--------|
| **Language** | Rust 2021 | Python 3.11+ | Python 3.10+ |
| **LOC** | ~93,500 + React frontend | ~238,400 + 101K tests | ~25,000 + SDKs |
| **Architecture** | Single static binary, 8 crates | Python package (uv/pip) | FastAPI service + worker |
| **Database** | SQLite (bundled) | SQLite + FTS5 | PostgreSQL + pgvector |
| **LLM Providers** | 8+ (Anthropic, OpenAI-compat, Ollama, Qwen) | 15+ (OpenRouter, Anthropic, OpenAI, Copilot, Google, GLM, Kimi, MiniMax, etc.) | 6 (Anthropic, Google, OpenAI, Groq, vLLM, OpenAI-compat) |
| **Chat Platforms** | 16 adapters | 13 adapters + OpenAI API server | None (API service) |
| **Tools** | 22+ built-in | 40+ built-in | 7 dialectic tools |
| **Skills** | ClawHub registry + 11 built-in | 100+ bundled + Skills Hub | N/A |
| **Config** | YAML (`mchact.config.yaml`) | YAML + .env (`~/.hermes/`) | TOML + env vars |
| **License** | MIT | MIT | AGPL-3.0 |
| **Focus** | All-in-one agent runtime | Agent framework + gateway | Memory infrastructure |

---

## Development Journey

The work followed a deliberate sequence:

### Phase 1: microclaw as Foundation
- Forked microclaw as the base (`f0efa57 feat: initialize mchact from microclaw upstream`)
- Inherited all 16 channel adapters, 22+ tools, embedded web UI, OTLP observability, scheduler, sub-agents, hooks, plugins, ClawHub, ACP support

### Phase 2: Feature-by-Feature Comparison (microclaw vs hermes-agent)
- Deep analysis of hermes-agent's architecture and capabilities
- Identified 5 high-impact features missing from microclaw
- Created brainstorm session with three-way comparison tables (microclaw vs hermes vs honcho)
- Documented in design spec: `docs/superpowers/specs/2026-03-27-fts5-search-compressor-moa-design.md`

### Phase 3: Port hermes-agent Features (IMPLEMENTED)
All 5 features shipped to main branch:

| Feature | Commits | What It Does |
|---------|---------|-------------|
| **FTS5 Session Search** | `59c58a0`..`3bab319` | `messages_fts` virtual table with auto-sync triggers, `session_search` tool, cross-session full-text search |
| **5-Phase Context Compressor** | `51ef86e` | Replaces single-pass compaction: tool pruning → head protection → token-budget tail → structured summary → iterative updates |
| **Anthropic Prompt Caching** | `c769f0a` | `cache_control` markers in serialization path, ~75% input cost reduction on cache hits |
| **MoA Shared Findings** | `018e577` | `subagent_findings` table, `findings_write`/`findings_read` tools for blackboard pattern |
| **Mixture of Agents** | `10bb18d` | `mixture_of_agents` tool via `SubagentsOrchestrateTool`, multi-perspective consensus |

### Phase 4: Honcho Alignment — mchact-memory Observation Engine (IMPLEMENTED)
Hermes uses honcho for cross-session user modeling. We built our own honcho-inspired observation engine natively:

| Feature | Commits | What It Does |
|---------|---------|-------------|
| **Crate scaffold + types** | `7ed4655` | `ObservationStore` trait, 4-level observations (explicit/deductive/inductive/contradiction), peer identity model |
| **Quality gates** | `4e6a125` | PII scan, poisoning guard, length validation, dedup |
| **SQLite + PostgreSQL schemas** | `a3c0170` | Dual-driver DDL with migrations |
| **SQLite driver** | `3b763d8` | Core CRUD, peer management |
| **Queue + findings + observability** | `c389fc7` | Async task queue, findings promotion, deriver/dreamer run logs |
| **RRF hybrid search** | `c088901` | FTS5 keyword + embedding semantic search with Reciprocal Rank Fusion merge |
| **DAG traversal** | `11e3c04` | Source attribution chains — trace conclusions back to premises |
| **Memory injection** | `830cbce` | Token-budgeted observation formatting for system prompts |
| **Deriver + dreamer** | `2f7756a` | Background extraction (multi-level) + offline consolidation (deduction/induction specialists) |
| **PostgreSQL driver** | `9306b95` | Full pgvector + tsvector support |
| **Driver factory + config** | `a4eecab` | Config-based SQLite/PostgreSQL driver selection |
| **AppState wiring** | `cde1a98` | `ObservationStore` integrated into runtime |

**Key design decisions from brainstorm session:**
- Chose "Unified Dual-Driver" approach (Option D) — one `ObservationStore` trait, two backends
- SQLite for zero-config deployment, PostgreSQL for advanced features (full DAG, native HNSW)
- Both drivers support all features (no degraded mode)
- Peer paradigm from honcho: unified user/agent identity model
- Dreamer runs during idle periods (like honcho's dream system)

### Phase 5: Multimodal Capabilities (IMPLEMENTED)
Inspired by hermes-agent's multimodal tools (TTS, STT, image gen, vision). Full spec + implementation:

| Feature | Commits | What It Does |
|---------|---------|-------------|
| **Media crate scaffold** | `374e4fa` | `crates/mchact-media/` with error types, provider router pattern |
| **Document storage** | `5b3ee4f` | Migration v21, `document_extractions` table, per-chat SHA-256 dedup |
| **Config fields** | `e1bdc74` | 28 new settings for TTS/STT/image/video/vision/documents |
| **TTS providers** | `29b3058` | Edge TTS (free), OpenAI TTS, ElevenLabs — `text_to_speech` tool |
| **Video gen providers** | `310245b` | Sora 2, FAL video, MiniMax Hailuo 2.3 — queue-based polling |
| **STT providers** | `4c59c3d` | OpenAI Whisper + local whisper-rs (Metal/CUDA) |
| **Image gen providers** | `ce40d36` | DALL-E + FAL FLUX — `image_generate` tool |
| **Web media components** | `a4459ab` | ImageViewer, AudioPlayer, VideoPlayer, FilePreview |
| **Channel adapters** | `53d18f0` | `send_voice()` / `send_video()` on ChannelAdapter trait |
| **Document tool** | `ebb37db` | `read_document` with kreuzberg extraction (91+ formats) |
| **Vision routing** | `350fb12` | Auto-fallback to OpenRouter when model lacks vision |
| **Media tools** | `d9a58e0` | `text_to_speech`, `image_generate`, `video_generate` tools registered |
| **Web attachments** | `29381c1` | Media attachment rendering in chat messages |
| **Web settings** | `23f9a3d` | Multimodal settings tab with provider configuration |
| **API endpoints** | `71784ab` | `/api/upload`, `/api/media/{id}`, media SSE events |
| **Setup wizard** | `88563a5` | Voice & Speech, Media Generation, Vision & Documents TUI pages |

### Phase 6: Rename (COMPLETED)
- Renamed from MicroClaw to mchact across all files, crates, configs, docs, web UI

---

## What's Been Ported — Complete Inventory

### From microclaw (Foundation — everything)
- Complete agent engine (`process_with_agent`)
- All 16 channel adapters (Telegram, Discord, Slack, Feishu, WeChat, Matrix, IRC, Email, Signal, WhatsApp, DingTalk, QQ, iMessage, Nostr, Web, ACP)
- All 22+ tools (file ops, bash, web, memory, scheduling, sub-agents, MCP, etc.)
- SQLite database with versioned migrations (now v21)
- Memory system (file-based AGENTS.md + structured SQLite)
- Scheduler with DLQ and timezone support
- Sub-agent orchestration with restricted tool registries
- Embedded React web UI
- OTLP observability (traces, metrics, logs)
- Hooks platform (BeforeLLMCall, BeforeToolCall, AfterToolCall)
- ClawHub skill registry
- Plugin system
- ACP support (stdio mode)
- Path guard security
- Sandbox execution (Docker/Podman/Bubblewrap)
- Setup wizard TUI and doctor diagnostics
- Session forking model
- Auth & authorization (session-cookie + API keys + scopes)

### From hermes-agent (5 features ported)
1. **FTS5 Session Search** — cross-session full-text search with context windows
2. **5-Phase Context Compressor** — intelligent multi-stage context management
3. **Mixture of Agents** — multi-perspective consensus via parallel sub-agents
4. **MoA Shared Findings Blackboard** — sub-agent collaboration via findings table
5. **Anthropic Prompt Caching** — cache_control markers for ~75% input cost reduction

### From honcho (Observation engine ported as mchact-memory)
1. **4-Level Observation Hierarchy** — explicit → deductive → inductive → contradiction
2. **Peer Identity Model** — unified user/agent modeling (peer paradigm)
3. **Peer Cards** — auto-maintained biographical summaries (max 40 facts)
4. **Source Attribution DAG** — trace conclusions back to premises
5. **Hybrid RRF Search** — keyword (FTS5/tsvector) + semantic (embeddings) merged via RRF
6. **Deriver Agent** — async multi-level observation extraction from conversations
7. **Dreamer Agent** — offline consolidation with deduction + induction specialists
8. **Quality Gates** — PII scan, poisoning guard, length validation, dedup
9. **Memory Injection** — token-budgeted observation formatting for system prompts
10. **Dual-Driver Architecture** — SQLite and PostgreSQL backends behind one trait
11. **Findings Promotion** — MoA findings promoted to long-term observations

### Multimodal (inspired by hermes, fresh implementation)
1. **TTS** — Edge TTS, OpenAI TTS, ElevenLabs providers
2. **STT** — OpenAI Whisper + local whisper-rs (Metal/CUDA)
3. **Image Generation** — DALL-E + FAL FLUX
4. **Video Generation** — Sora 2, FAL video, MiniMax Hailuo 2.3
5. **Document Intelligence** — kreuzberg extraction (91+ formats), per-chat storage
6. **Vision Routing** — auto-fallback to OpenRouter for vision-capable models
7. **Web Media Components** — ImageViewer, AudioPlayer, VideoPlayer, FilePreview
8. **Channel Media Adapters** — send_voice/send_video on ChannelAdapter trait

---

## What Remains — Not Yet Ported

### From hermes-agent

| Feature | Category | Impact | Complexity | Notes |
|---------|----------|--------|------------|-------|
| **OpenRouter provider** | LLM | High | Low | Instant access to 200+ models |
| **GitHub Copilot provider** | LLM | Medium | Medium | Free for Copilot subscribers, 400K context |
| **Google/Vertex provider** | LLM | Medium | Medium | Gemini models |
| **GLM/ZhipuAI provider** | LLM | Low | Low | Chinese market |
| **Kimi/Moonshot provider** | LLM | Low | Low | Chinese market |
| **MiniMax provider** | LLM | Low | Low | Chinese market |
| **RL training pipeline** | Agent | Low | Very High | Tinker-Atropos integration, trajectory generation, WandB monitoring. Highly specialized — only relevant if training custom models |
| **Memory injection scanning** | Security | Medium | Low | Prompt injection / exfiltration pattern detection before memory writes |
| **KV-cache stable memory** | Performance | Medium | Low | Freeze memory snapshot at session start to preserve LLM prefix cache |
| **Smart model routing** | Performance | Medium | Medium | Route simple queries to cheaper models via heuristics |
| **Code execution sandbox** | Tools | Low | Medium | Isolated Python execution (mchact has bash with Docker sandbox) |
| **Vision analysis tool** | Tools | Low | Low | Standalone image analysis (mchact has vision routing) |
| **Home Assistant integration** | Platform | Low | Medium | 4 HA tools (list entities, get state, list/call services) |
| **Mattermost adapter** | Platform | Low | Medium | Enterprise chat |
| **SMS (Twilio) adapter** | Platform | Low | Low | Text messaging |
| **OpenAI-compatible API server** | Platform | Medium | Medium | Expose `/v1/chat/completions` endpoint |
| **Webhook trigger platform** | Platform | Low | Low | External event triggers |
| **Terminal backends** | Execution | Low | High | SSH, Modal, Daytona, Singularity (mchact has Docker/Podman/Bubblewrap) |
| **Skill auto-creation** | Skills | Low | Medium | Agent autonomously creates SKILL.md after complex tasks |
| **Per-session cost tracking** | Observability | Low | Low | Usage-aware cost projection |

### From honcho (Additional features beyond what's in mchact-memory)

| Feature | Category | Impact | Complexity | Notes |
|---------|----------|--------|------------|-------|
| **Surprisal sampling** | Memory | Low | Low | Geometric surprisal filter for dream focus — optimization |
| **Contradiction detection** | Memory | Medium | Medium | Automatic detection of conflicting observations |
| **Session-scoped observation config** | Memory | Low | Medium | Per-peer-per-session observation settings |
| **Dialectic chat API** | API | Medium | Medium | Natural language querying of memory via tool-using agent |
| **Webhook delivery** | Integration | Low | Low | Webhook notifications on memory events |
| **Langfuse integration** | Observability | Low | Low | Tracing (mchact has OTLP which covers this) |
| **Redis caching** | Performance | Low | Low | Cache layer (SQLite is fast enough for most deployments) |

---

## Architecture Comparison

### Memory Architecture (Three-Way)

| Aspect | microclaw (original) | hermes-agent | honcho | mchact (current) |
|--------|---------------------|--------------|--------|-----------------|
| **Storage** | SQLite + vector-search | SQLite WAL + files | PostgreSQL + pgvector | SQLite OR PostgreSQL (dual-driver, vector-search) |
| **Model** | Flat (3 categories) | Dual-layer (files + sessions) | 4-level hierarchy | **4-level hierarchy** (from honcho) |
| **Extraction** | Reflector loop (every 30min) | Agent self-curates | Deriver (async queue) | **Deriver agent** (from honcho) |
| **Consolidation** | Jaccard dedup | Manual | Dreamer (deduction + induction) | **Dreamer agent** (from honcho) |
| **Search** | Keyword only | FTS5 + LLM summary | Hybrid RRF | **Hybrid RRF** (from honcho) |
| **Identity** | chat_id only | user_id per platform | Unified peers | **Peer paradigm** (from honcho) |
| **Reasoning chain** | Flat supersede edges | None | Full DAG | **Source DAG** (from honcho) |
| **Profile** | AGENTS.md per-chat | USER.md + Honcho | Peer cards (40 facts) | **Peer cards** (from honcho) |
| **Security** | Quality gates | PII + injection scan | JWT + workspace isolation | **Quality gates + PII** (combined) |

### Context Management

| Aspect | microclaw (original) | hermes-agent | mchact (current) |
|--------|---------------------|--------------|-----------------|
| **Compaction** | Single-pass summary | 5-phase structured | **5-phase** (from hermes) |
| **Tool pruning** | None | Phase 1 | **Phase 1** (from hermes) |
| **Head/tail protection** | None | Phases 2-3 | **Phases 2-3** (from hermes) |
| **Prompt caching** | None | system_and_3 strategy | **cache_control markers** (from hermes) |
| **Cross-session search** | None | FTS5 + LLM summary | **FTS5** (from hermes) |

### Tool Ecosystem

| Category | microclaw | hermes | mchact (current) |
|----------|-----------|--------|-----------------|
| **File ops** | 5 tools | 4 tools | **5 tools** (microclaw) |
| **Shell** | bash (Docker sandbox) | terminal (6 backends) | **bash** (Docker/Podman/Bubblewrap) |
| **Web** | fetch + search | extract + search | **fetch + search** (microclaw) |
| **Browser** | 1 tool | 11 tools | **1 tool** (microclaw) |
| **Memory** | 5 tools | 1 tool + honcho | **5 tools + observation store** |
| **Scheduling** | 8 tools | cronjob | **8 tools** (microclaw) |
| **Sub-agents** | 9 tools | delegate_task | **9 tools + MoA** (microclaw + hermes) |
| **Multimodal** | None | TTS + STT + image + vision | **TTS + STT + image + video + docs + vision** |
| **RL training** | N/A | 10 tools | **Not ported** |
| **Home automation** | N/A | 4 tools | **Not ported** |

---

## Architectural Lessons Incorporated

### From microclaw
- Single binary deployment (zero runtime deps)
- Crate-boundary separation prevents spaghetti
- SQLite scales to millions of messages
- Embed the web UI — no separate deployment

### From hermes-agent
- FTS5 is cheap and powerful for session continuity
- 5-phase compression preserves structure better than single-pass
- Prompt caching reduces costs dramatically
- Multi-perspective consensus (MoA) improves answer quality

### From honcho
- Memory deserves hierarchical reasoning, not just flat facts
- Dreams (proactive consolidation) create emergent insight
- Peer paradigm simplifies multi-agent scenarios
- Tool-using dialectic agent produces better memory queries than direct retrieval
- Dual-driver architecture lets deployment scale with infrastructure

---

## Design Artifacts

| Document | Path | Purpose |
|----------|------|---------|
| FTS5 + Compressor + MoA Spec | `docs/superpowers/specs/2026-03-27-fts5-search-compressor-moa-design.md` | Design for hermes features |
| FTS5 + Compressor + MoA Plan | `docs/superpowers/plans/2026-03-27-fts5-compressor-moa-cache.md` | Implementation roadmap |
| mchact-memory Spec | `docs/superpowers/specs/2026-03-27-mchact-memory-design.md` | Honcho-inspired observation engine |
| mchact-memory Plan | `docs/superpowers/plans/2026-03-27-mchact-memory.md` | Implementation roadmap |
| Multimodal Spec | `docs/superpowers/specs/2026-03-27-multimodal-design.md` | TTS/STT/image/video/docs/vision |
| Multimodal Plan | `docs/superpowers/plans/2026-03-27-multimodal.md` | Implementation roadmap |
| Brainstorm Session | `.superpowers/brainstorm/52667-1774599566/` | Three-way comparison, architecture options |

### RFCs (all implemented)
| RFC | Status | Feature |
|-----|--------|---------|
| RFC-0001 | Shipped | Web auth & authorization |
| RFC-0002 | Shipped | Hooks platform MVP |
| RFC-0003 | Shipped | Session forking model |
| RFC-0004 | Shipped | Metrics & tracing naming |
| RFC-0005 | In Progress | Session-native subagents V1 |

---

## Recommended Next Steps

### High Impact, Low Effort
1. **OpenRouter provider** — unlock 200+ models with one integration
2. **KV-cache stable memory** — freeze memory at session start for prompt cache stability
3. **Memory injection scanning** — detect prompt injection/exfiltration in memory writes

### Medium Impact, Medium Effort
4. **Smart model routing** — route simple queries to cheaper models
5. **OpenAI-compatible API server** — expose `/v1/chat/completions` for external integrations
6. **Dialectic chat API** — natural language memory querying (tool-using agent approach from honcho)

### Specialized / Lower Priority
7. **Home Assistant integration** — smart home control
8. **Additional terminal backends** — SSH, Modal for remote/serverless execution
9. **RL training pipeline** — only if training custom models (Tinker-Atropos from hermes)
10. **Skill auto-creation** — agent writes SKILL.md after complex tasks
