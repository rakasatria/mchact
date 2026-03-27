import type { A2APeerDraft } from "./types";
import { DOCS_BASE, BOT_SLOT_MAX } from "./constants";

export function warningDocUrl(code?: string): string {
  switch (code) {
    case "sandbox_disabled":
    case "sandbox_runtime_unavailable":
    case "sandbox_mount_allowlist_missing":
      return `${DOCS_BASE}/configuration#sandbox`;
    case "auth_password_not_configured":
    case "web_host_not_loopback":
      return `${DOCS_BASE}/permissions`;
    case "web_rate_limit_too_high":
    case "web_inflight_limit_too_high":
    case "web_rate_window_too_small_for_limit":
    case "web_session_idle_ttl_too_low":
      return `${DOCS_BASE}/configuration#web`;
    case "hooks_max_input_bytes_too_high":
    case "hooks_max_output_bytes_too_high":
      return `${DOCS_BASE}/hooks`;
    case "otlp_enabled_without_endpoint":
    case "otlp_queue_capacity_low":
    case "otlp_retry_attempts_too_low":
      return `${DOCS_BASE}/observability`;
    default:
      return `${DOCS_BASE}/configuration`;
  }
}

export function defaultModelForProvider(providerRaw: string): string {
  const provider = providerRaw.trim().toLowerCase();
  if (provider === "anthropic") return "claude-sonnet-4-5-20250929";
  if (provider === "openai-codex") return "gpt-5.3-codex";
  if (provider === "ollama") return "llama3.2";
  if (provider === "google") return "gemini-2.5-pro";
  if (provider === "aliyun-bailian") return "qwen3.5-plus";
  if (provider === "nvidia") return "meta/llama-3.3-70b-instruct";
  return "gpt-5.2";
}

export function normalizeAccountId(raw: unknown): string {
  const text = String(raw || "").trim();
  return text || "main";
}

export function defaultAccountIdFromChannelConfig(channelCfg: unknown): string {
  if (!channelCfg || typeof channelCfg !== "object") return "main";
  const cfg = channelCfg as Record<string, unknown>;
  const explicit = String(cfg.default_account || "").trim();
  if (explicit) return explicit;
  const accounts = cfg.accounts;
  if (accounts && typeof accounts === "object") {
    const keys = Object.keys(accounts as Record<string, unknown>).sort();
    if (keys.includes("default")) return "default";
    if (keys.length > 0) return keys[0] || "main";
  }
  return "main";
}

export function defaultAccountConfig(channelCfg: unknown): Record<string, unknown> {
  if (!channelCfg || typeof channelCfg !== "object") return {};
  const cfg = channelCfg as Record<string, unknown>;
  const accountId = defaultAccountIdFromChannelConfig(cfg);
  const accounts = cfg.accounts;
  if (!accounts || typeof accounts !== "object") return {};
  const account = (accounts as Record<string, unknown>)[accountId];
  return account && typeof account === "object"
    ? (account as Record<string, unknown>)
    : {};
}

export function defaultTelegramAccountIdForSlot(slot: number): string {
  return slot <= 1 ? "main" : `bot${slot}`;
}

export function defaultAccountIdForSlot(slot: number): string {
  return slot <= 1 ? "main" : `bot${slot}`;
}

export function normalizeBotCount(raw: unknown): number {
  const n = Number(raw);
  if (!Number.isFinite(n)) return 1;
  return Math.min(BOT_SLOT_MAX, Math.max(1, Math.floor(n)));
}

export function normalizeSoulPathInput(raw: unknown, soulsDir?: unknown): string {
  const trimmed = String(raw || "").trim();
  if (!trimmed) return "";
  if (trimmed.includes("/") || trimmed.includes("\\")) return trimmed;
  const base =
    String(soulsDir || "")
      .trim()
      .replace(/[\\/]+$/, "") || "souls";
  if (trimmed.toLowerCase().endsWith(".md")) return `${base}/${trimmed}`;
  return `${base}/${trimmed}.md`;
}

export function soulFileNameFromPath(raw: unknown): string {
  const text = String(raw || "").trim();
  if (!text) return "";
  const normalized = text.replace(/\\/g, "/");
  const parts = normalized.split("/");
  return parts[parts.length - 1] || "";
}

export function soulPickerValue(
  raw: unknown,
  options: readonly string[],
  soulsDir?: unknown,
): string {
  const normalized = normalizeSoulPathInput(raw, soulsDir);
  if (!normalized) return "__none__";
  const fileName = soulFileNameFromPath(normalized);
  return options.includes(fileName) ? fileName : "__custom__";
}

export function orderedAccountsFromChannelConfig(
  channelCfg: unknown,
): Array<[string, Record<string, unknown>]> {
  if (!channelCfg || typeof channelCfg !== "object") return [];
  const cfg = channelCfg as Record<string, unknown>;
  const accountsRaw = cfg.accounts;
  if (!accountsRaw || typeof accountsRaw !== "object") return [];
  const accountsObj = accountsRaw as Record<string, unknown>;
  const entries: Array<[string, Record<string, unknown>]> = Object.entries(
    accountsObj,
  )
    .filter(([, v]) => v && typeof v === "object" && !Array.isArray(v))
    .map(([id, v]) => [id, v as Record<string, unknown>]);
  if (entries.length === 0) return [];

  const defaultId = defaultAccountIdFromChannelConfig(cfg);
  entries.sort(([a], [b]) => a.localeCompare(b));
  const defaultIdx = entries.findIndex(([id]) => id === defaultId);
  if (defaultIdx > 0) {
    const [defaultEntry] = entries.splice(defaultIdx, 1);
    entries.unshift(defaultEntry);
  }
  return entries.slice(0, BOT_SLOT_MAX);
}

export function orderedTelegramAccountsFromChannelConfig(
  channelCfg: unknown,
): Array<[string, Record<string, unknown>]> {
  return orderedAccountsFromChannelConfig(channelCfg);
}

export function normalizeWorkingDirIsolation(value: unknown): "chat" | "shared" {
  const normalized = String(value || "")
    .trim()
    .toLowerCase();
  return normalized === "shared" ? "shared" : "chat";
}

export function parseDiscordChannelCsv(input: string): number[] {
  const out: number[] = [];
  for (const part of input.split(",")) {
    const trimmed = part.trim();
    if (!trimmed) continue;
    const n = Number(trimmed);
    if (Number.isInteger(n) && n > 0) {
      out.push(n);
    }
  }
  return Array.from(new Set(out));
}

export function parseI64ListCsvOrJsonArray(
  input: string,
  fieldName: string,
): number[] {
  const trimmed = input.trim();
  if (!trimmed) return [];

  const parsedAsCsv = (): number[] => {
    const out: number[] = [];
    for (const part of trimmed.split(",")) {
      const token = part.trim();
      if (!token) continue;
      if (!/^-?\d+$/.test(token)) {
        throw new Error(
          `${fieldName} must be a CSV of integers or a JSON integer array`,
        );
      }
      const n = Number(token);
      if (!Number.isSafeInteger(n)) {
        throw new Error(`${fieldName} contains an out-of-range integer`);
      }
      out.push(n);
    }
    return Array.from(new Set(out));
  };

  if (trimmed.startsWith("[")) {
    let parsed: unknown;
    try {
      parsed = JSON.parse(trimmed);
    } catch (e) {
      throw new Error(
        `${fieldName} must be valid JSON array: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
    if (!Array.isArray(parsed)) {
      throw new Error(
        `${fieldName} must be a JSON array when using JSON format`,
      );
    }
    const out: number[] = [];
    for (const item of parsed) {
      if (typeof item !== "number" || !Number.isSafeInteger(item)) {
        throw new Error(`${fieldName} JSON array must contain integers only`);
      }
      out.push(item);
    }
    return Array.from(new Set(out));
  }

  return parsedAsCsv();
}

export function parseStringListInput(input: string): string[] {
  const trimmed = input.trim();
  if (!trimmed) return [];
  if (trimmed.startsWith("[")) {
    let parsed: unknown;
    try {
      parsed = JSON.parse(trimmed);
    } catch (e) {
      throw new Error(
        `a2a_shared_tokens must be valid JSON array: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
    if (!Array.isArray(parsed)) {
      throw new Error(
        "a2a_shared_tokens must be a JSON array when using JSON format",
      );
    }
    return Array.from(
      new Set(parsed.map((item) => String(item || "").trim()).filter(Boolean)),
    );
  }
  return Array.from(
    new Set(
      trimmed
        .split(",")
        .map((item) => item.trim())
        .filter(Boolean),
    ),
  );
}

export function peersFromConfigValue(value: unknown): A2APeerDraft[] {
  if (!value || typeof value !== "object" || Array.isArray(value)) return [];
  return Object.entries(value as Record<string, unknown>)
    .map(([name, raw]) => {
      const peer =
        raw && typeof raw === "object" && !Array.isArray(raw)
          ? (raw as Record<string, unknown>)
          : {};
      const bearer = String(peer.bearer_token || "").trim();
      return {
        name,
        enabled: peer.enabled !== false,
        base_url: String(peer.base_url || ""),
        bearer_token: "",
        has_bearer_token: Boolean(bearer),
        description: String(peer.description || ""),
        default_session_key: String(peer.default_session_key || ""),
      };
    })
    .sort((a, b) => a.name.localeCompare(b.name));
}

export function emptyA2APeer(): A2APeerDraft {
  return {
    name: "",
    enabled: true,
    base_url: "",
    bearer_token: "",
    has_bearer_token: false,
    description: "",
    default_session_key: "",
  };
}

export function parseOptionalBoolString(
  input: string,
  fieldName: string,
): boolean | null {
  const trimmed = input.trim().toLowerCase();
  if (!trimmed) return null;
  if (trimmed === "true" || trimmed === "1" || trimmed === "yes") return true;
  if (trimmed === "false" || trimmed === "0" || trimmed === "no") return false;
  throw new Error(`${fieldName} must be true/false (or 1/0)`);
}

export function parseOptionalU64String(
  input: string,
  fieldName: string,
): number | null {
  const trimmed = input.trim();
  if (!trimmed) return null;
  if (!/^\d+$/.test(trimmed)) {
    throw new Error(`${fieldName} must be a non-negative integer`);
  }
  const parsed = Number(trimmed);
  if (!Number.isSafeInteger(parsed)) {
    throw new Error(`${fieldName} must be a safe integer`);
  }
  return parsed;
}

export function dynamicFieldDraftValue(
  raw: unknown,
  valueType: "string" | "bool" | "number" = "string",
): string {
  if (valueType === "bool") {
    if (typeof raw === "boolean") return raw ? "true" : "false";
    const text = String(raw || "")
      .trim()
      .toLowerCase();
    if (!text) return "";
    if (text === "true" || text === "1" || text === "yes") return "true";
    if (text === "false" || text === "0" || text === "no") return "false";
    return String(raw || "");
  }
  if (valueType === "number") {
    if (typeof raw === "number" && Number.isFinite(raw))
      return String(Math.trunc(raw));
    const text = String(raw || "").trim();
    if (!text) return "";
    return text;
  }
  return String(raw || "");
}
