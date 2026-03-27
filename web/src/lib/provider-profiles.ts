import type { ConfigPayload, ProviderProfileDraft } from "./types";
import { BOT_SLOT_MAX, MAIN_PROFILE_VALUE } from "./constants";
import { DYNAMIC_CHANNELS } from "./channels";
import {
  defaultModelForProvider,
  normalizeAccountId,
  defaultTelegramAccountIdForSlot,
  defaultAccountIdForSlot,
  normalizeBotCount,
} from "./config-helpers";

export function nextProviderProfileId(entries: ProviderProfileDraft[]): string {
  const used = new Set(
    entries
      .map((entry) =>
        String(entry.id || "")
          .trim()
          .toLowerCase(),
      )
      .filter(Boolean),
  );
  for (let idx = 1; idx < 10_000; idx += 1) {
    const candidate = `provider${idx}`;
    if (!used.has(candidate)) return candidate;
  }
  return "provider1";
}

export function nextClonedProviderProfileId(
  entries: ProviderProfileDraft[],
  sourceId: string,
): string {
  const base = String(sourceId || "")
    .trim()
    .toLowerCase();
  if (!base) return nextProviderProfileId(entries);
  const used = new Set(
    entries
      .map((entry) =>
        String(entry.id || "")
          .trim()
          .toLowerCase(),
      )
      .filter(Boolean),
  );
  for (let idx = 2; idx < 10_000; idx += 1) {
    const candidate = `${base}-${idx}`;
    if (!used.has(candidate)) return candidate;
  }
  return `${base}-2`;
}

export function emptyProviderProfileDraft(
  entries: ProviderProfileDraft[],
): ProviderProfileDraft {
  return {
    id: nextProviderProfileId(entries),
    provider: "anthropic",
    api_key: "",
    llm_base_url: "",
    llm_user_agent: "",
    default_model: defaultModelForProvider("anthropic"),
    show_thinking: false,
  };
}

export function normalizeProviderProfileDraft(
  raw: unknown,
  fallbackId = "",
): ProviderProfileDraft {
  const draft =
    raw && typeof raw === "object" ? (raw as Record<string, unknown>) : {};
  return {
    id: String(draft.id || fallbackId || "").trim(),
    provider: String(draft.provider || "").trim(),
    api_key:
      typeof draft.api_key === "string" && draft.api_key.trim() === "***"
        ? ""
        : String(draft.api_key || ""),
    llm_base_url: String(draft.llm_base_url || ""),
    llm_user_agent: String(draft.llm_user_agent || ""),
    default_model: String(draft.default_model || ""),
    show_thinking: Boolean(draft.show_thinking),
  };
}

export function providerProfilesFromConfig(
  config: ConfigPayload | null,
): ProviderProfileDraft[] {
  const presetsRaw =
    (config?.provider_presets as Record<string, unknown> | undefined) ||
    (config?.llm_providers as Record<string, unknown> | undefined) ||
    {};
  return Object.entries(presetsRaw)
    .filter(([id]) => id.trim() && id.trim().toLowerCase() !== "main")
    .map(([id, value]) => normalizeProviderProfileDraft(value, id))
    .sort((a, b) => a.id.localeCompare(b.id));
}

export function serializeProviderProfiles(
  entries: ProviderProfileDraft[],
): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const raw of entries) {
    const entry = normalizeProviderProfileDraft(raw);
    const id = entry.id.trim().toLowerCase();
    if (!id || id === "main") continue;
    out[id] = {
      ...(entry.provider.trim()
        ? { provider: entry.provider.trim().toLowerCase() }
        : {}),
      ...(entry.api_key.trim() ? { api_key: entry.api_key.trim() } : {}),
      ...(entry.llm_base_url.trim()
        ? { llm_base_url: entry.llm_base_url.trim() }
        : {}),
      ...(entry.llm_user_agent.trim()
        ? { llm_user_agent: entry.llm_user_agent.trim() }
        : {}),
      ...(entry.default_model.trim()
        ? { default_model: entry.default_model.trim() }
        : {}),
      show_thinking: Boolean(entry.show_thinking),
    };
  }
  return out;
}

export function providerPresetFromConfigValue(raw: unknown): string {
  if (!raw || typeof raw !== "object") return "";
  const cfg = raw as Record<string, unknown>;
  return String(cfg.provider_preset || cfg.llm_provider || "").trim();
}

export function providerProfileOptions(
  entries: ProviderProfileDraft[],
  currentRaw: unknown,
): Array<{ value: string; label: string }> {
  const current = String(currentRaw || "").trim();
  const options = [
    { value: MAIN_PROFILE_VALUE, label: "main (global default)" },
  ];
  const seen = new Set<string>([MAIN_PROFILE_VALUE]);
  for (const entry of entries) {
    const id = String(entry.id || "").trim();
    if (!id || seen.has(id)) continue;
    options.push({
      value: id,
      label: `${id} · ${String(entry.provider || "custom").trim() || "custom"} / ${String(entry.default_model || "").trim() || "(no model)"}`,
    });
    seen.add(id);
  }
  if (current && !seen.has(current)) {
    options.push({ value: current, label: `${current} · custom/current` });
  }
  return options;
}

export function providerProfileReferences(
  configDraft: Record<string, unknown>,
  profileIdRaw: unknown,
): string[] {
  const profileId = String(profileIdRaw || "").trim();
  if (!profileId) return [];
  const refs: string[] = [];

  if (
    String(configDraft.telegram_provider_preset || "")
      .trim()
      .toLowerCase() === profileId.toLowerCase()
  ) {
    refs.push("telegram channel");
  }
  if (
    String(configDraft.discord_provider_preset || "")
      .trim()
      .toLowerCase() === profileId.toLowerCase()
  ) {
    refs.push("discord channel");
  }

  for (
    let slot = 1;
    slot <= normalizeBotCount(configDraft.telegram_bot_count || 1);
    slot += 1
  ) {
    if (
      String(configDraft[`telegram_bot_${slot}_provider_preset`] || "")
        .trim()
        .toLowerCase() === profileId.toLowerCase()
    ) {
      const accountId = normalizeAccountId(
        configDraft[`telegram_bot_${slot}_account_id`] ||
          defaultTelegramAccountIdForSlot(slot),
      );
      refs.push(`telegram.${accountId}`);
    }
  }

  for (
    let slot = 1;
    slot <= normalizeBotCount(configDraft.discord_bot_count || 1);
    slot += 1
  ) {
    if (
      String(configDraft[`discord_bot_${slot}_provider_preset`] || "")
        .trim()
        .toLowerCase() === profileId.toLowerCase()
    ) {
      const accountId = normalizeAccountId(
        configDraft[`discord_bot_${slot}_account_id`] ||
          defaultAccountIdForSlot(slot),
      );
      refs.push(`discord.${accountId}`);
    }
  }

  if (
    String(configDraft.irc_provider_preset || "")
      .trim()
      .toLowerCase() === profileId.toLowerCase()
  ) {
    refs.push("irc channel");
  }

  for (const ch of DYNAMIC_CHANNELS) {
    for (
      let slot = 1;
      slot <= normalizeBotCount(configDraft[`${ch.name}__bot_count`] || 1);
      slot += 1
    ) {
      const stateKey = `${ch.name}__bot_${slot}__provider_preset`;
      if (
        String(configDraft[stateKey] || "")
          .trim()
          .toLowerCase() === profileId.toLowerCase()
      ) {
        const accountId = normalizeAccountId(
          configDraft[`${ch.name}__bot_${slot}__account_id`] ||
            defaultAccountIdForSlot(slot),
        );
        refs.push(`${ch.name}.${accountId}`);
      }
    }
  }

  return Array.from(new Set(refs)).sort((a, b) => a.localeCompare(b));
}

export function renameProviderProfileReferences(
  configDraft: Record<string, unknown>,
  oldIdRaw: unknown,
  newIdRaw: unknown,
): Record<string, unknown> {
  const oldId = String(oldIdRaw || "").trim();
  const newId = String(newIdRaw || "").trim();
  if (!oldId || oldId.toLowerCase() === newId.toLowerCase()) return configDraft;

  const next: Record<string, unknown> = { ...configDraft };
  const maybeReplace = (key: string): void => {
    if (
      String(next[key] || "")
        .trim()
        .toLowerCase() === oldId.toLowerCase()
    ) {
      next[key] = newId;
    }
  };

  maybeReplace("telegram_provider_preset");
  maybeReplace("discord_provider_preset");
  for (let slot = 1; slot <= BOT_SLOT_MAX; slot += 1) {
    maybeReplace(`telegram_bot_${slot}_provider_preset`);
    maybeReplace(`discord_bot_${slot}_provider_preset`);
  }
  maybeReplace("irc_provider_preset");
  for (const ch of DYNAMIC_CHANNELS) {
    for (let slot = 1; slot <= BOT_SLOT_MAX; slot += 1) {
      maybeReplace(`${ch.name}__bot_${slot}__provider_preset`);
    }
  }
  return next;
}

export function resetProviderProfileReferencesToMain(
  configDraft: Record<string, unknown>,
  profileIdRaw: unknown,
): { nextDraft: Record<string, unknown>; resetRefs: string[] } {
  const profileId = String(profileIdRaw || "").trim();
  if (!profileId) return { nextDraft: configDraft, resetRefs: [] };

  const refs = providerProfileReferences(configDraft, profileId);
  if (refs.length === 0) return { nextDraft: configDraft, resetRefs: [] };

  const next = renameProviderProfileReferences(configDraft, profileId, "");
  return { nextDraft: next, resetRefs: refs };
}
