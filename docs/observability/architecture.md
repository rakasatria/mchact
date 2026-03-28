# Observability Architecture

This document describes the current observability design implemented by `mchact-observability` and the runtime integrations in `src/runtime.rs`, `src/agent_engine.rs`, and `src/web.rs`.

## Goals

- Export metrics, traces, and logs through a shared OpenTelemetry-oriented path.
- Keep exporter configuration centralized in config rather than scattering vendor logic through business code.
- Make vendor-specific behavior adapter-driven.
- Preserve runtime safety with bounded queues and retry/backoff settings where exporters can fail under load.

## Architecture Layers

```text
mchact runtime
  ├─ agent engine spans and events
  ├─ web request / run / metrics snapshots
  ├─ scheduler and background jobs
  └─ runtime startup/shutdown signals
        |
        v
crates/mchact-observability
  ├─ sdk.rs         # shared config parsing and OTel resource setup
  ├─ metrics.rs     # OTLP metrics exporter
  ├─ traces.rs      # OTLP traces exporter
  ├─ logs.rs        # OTLP logs exporter
  └─ adapters/      # vendor endpoint/header adapters
        |
        v
OTLP receiver or vendor endpoint
```

## Runtime Ownership

- `src/runtime.rs`
  - Instantiates `OtlpMetricExporter`, `OtlpTraceExporter`, and `OtlpLogExporter` when enabled.
  - Stores them on `AppState`.

- `src/web.rs`
  - Accumulates HTTP, request, tool, and MCP counters in `WebMetrics`.
  - Periodically snapshots/export those counters through the metrics exporter.

- `src/agent_engine.rs`
  - Emits spans around agent runs, LLM generations, and tool execution.

- `src/scheduler.rs` and related background modules
  - Reuse the same exporter objects attached to `AppState`.

## Signal Types

### Metrics

Metrics are used for:

- HTTP request volume and error rate
- request latency
- LLM completion and token counts
- tool success/error/policy-block counts
- MCP reliability counters
- persisted history snapshots surfaced through `/api/metrics` and `/api/metrics/history`

The web layer is the main in-process metrics producer, but metrics history is also persisted to the database for trend views and SLO summaries.

### Traces

Traces are used to represent execution structure:

- agent runs
- LLM calls
- tool calls
- request lifecycles and other nested operations where span structure matters

This is the signal best suited to external tracing backends such as Langfuse or AgentOps.

### Logs

Logs are exported separately from traces and metrics when OTLP log export is enabled.

## Configuration Model

Observability config lives under the `observability:` block in `mchact.config.yaml`.

Example:

```yaml
observability:
  service_name: "mchact"
  otlp_headers:
    x-tenant-id: "tenant-a"

  otlp_enabled: true
  otlp_endpoint: "http://127.0.0.1:4318/v1/metrics"
  otlp_export_interval_seconds: 15

  otlp_tracing_enabled: true
  otlp_tracing_endpoint: "http://127.0.0.1:4318/v1/traces"

  otlp_logs_enabled: false
  otlp_logs_endpoint: "http://127.0.0.1:4318/v1/logs"
```

Important runtime tunables also exist for queueing and retry behavior, including:

- `otlp_queue_capacity`
- `otlp_retry_max_attempts`
- `otlp_retry_base_ms`
- `otlp_retry_max_ms`

These matter when metrics/traces/logs are exported over unreliable or rate-limited networks.

## Vendor Adapters

Vendor-specific logic is intentionally narrow.

- Adapters inject endpoint defaults and auth headers.
- They do not own business signal generation.
- They should not mutate runtime event semantics.

Current adapter paths inferred from code and config:

- Langfuse
  - derives trace endpoint from `langfuse_host`
  - builds a Basic auth header from `langfuse_public_key` and `langfuse_secret_key`

- AgentOps
  - uses `agentops_api_key`
  - can use an explicit `agentops_otlp_endpoint`

> ⚠️ Needs clarification: exact vendor routing precedence for every mixed configuration combination should be verified from `mchact-observability` source when extending this layer further. The current repo clearly supports Langfuse and AgentOps, but does not document a broader stable adapter contract outside the crate itself.

## End-to-End Data Flow

### Metrics flow

1. Runtime builds `OtlpMetricExporter`.
2. Web/server code records counters and latency samples.
3. A snapshot is converted to OpenTelemetry metrics.
4. The exporter batches and flushes to the OTLP endpoint on the configured interval.

### Trace flow

1. Runtime builds `OtlpTraceExporter`.
2. Agent/web/background code emits internal span data.
3. Spans are mapped into OTel structures.
4. The exporter sends them to the explicit trace endpoint or vendor-derived target.

### Log flow

1. Runtime builds `OtlpLogExporter` when enabled.
2. Structured runtime log events are mapped into OTel log records.
3. Batch export pushes them to the configured logs endpoint.

## Operational Boundaries

- Exporters are optional and must fail without taking down the runtime.
- Metrics history in the DB is separate from OTLP export.
- OTLP export is outbound-only; the project does not ship its own collector.
- Observability is runtime-wide. It is not scoped per channel instance.

## Extending the Layer

When adding a new vendor adapter:

1. Add the adapter under `crates/mchact-observability/src/adapters/`.
2. Keep it limited to endpoint/header derivation.
3. Expose config fields in `mchact.config.example.yaml`.
4. Update docs and validation examples.
5. Verify with `cargo test`, `cargo clippy --all-targets`, and a live OTLP smoke test.
