import { useMemo, useRef, useState } from "react";

import { api } from "../lib/api";
import type {
  ConfigPayload,
  ConfigSelfCheck,
  A2APeerDraft,
  ProviderProfileDraft,
  Appearance,
} from "../lib/types";

import { DEFAULT_CONFIG_VALUES, BOT_SLOT_MAX } from "../lib/constants";

import { DYNAMIC_CHANNELS } from "../lib/channels";

import {
  defaultModelForProvider,
  defaultAccountIdFromChannelConfig,
  defaultAccountConfig,
  defaultTelegramAccountIdForSlot,
  defaultAccountIdForSlot,
  normalizeBotCount,
  normalizeWorkingDirIsolation,
  orderedAccountsFromChannelConfig,
  orderedTelegramAccountsFromChannelConfig,
  peersFromConfigValue,
  dynamicFieldDraftValue,
  emptyA2APeer,
} from "../lib/config-helpers";

import { applyConfigFieldReset } from "../lib/config-reset";
import { buildConfigPayload } from "../lib/config-serializer";

import {
  normalizeProviderProfileDraft,
  providerProfilesFromConfig,
  providerPresetFromConfigValue,
  providerProfileReferences,
  renameProviderProfileReferences,
  resetProviderProfileReferencesToMain,
  emptyProviderProfileDraft,
  nextClonedProviderProfileId,
} from "../lib/provider-profiles";

export type UseConfigDeps = {
  appearance: Appearance;
  isUnauthorizedError: (err: unknown) => boolean;
  isForbiddenError: (err: unknown) => boolean;
  lockForAuth: (message?: string) => void;
};

export function useConfig(deps: UseConfigDeps) {
  const { appearance, isUnauthorizedError, isForbiddenError, lockForAuth } =
    deps;

  const [configOpen, setConfigOpen] = useState<boolean>(false);
  const [configLoading, setConfigLoading] = useState<boolean>(false);
  const [configLoadStage, setConfigLoadStage] = useState<string>("");
  const [configLoadError, setConfigLoadError] = useState<string>("");
  const [config, setConfig] = useState<ConfigPayload | null>(null);
  const [configDraft, setConfigDraft] = useState<Record<string, unknown>>({});
  const [soulFiles, setSoulFiles] = useState<string[]>([]);
  const [configSelfCheck, setConfigSelfCheck] =
    useState<ConfigSelfCheck | null>(null);
  const [configSelfCheckLoading, setConfigSelfCheckLoading] =
    useState<boolean>(false);
  const [configSelfCheckError, setConfigSelfCheckError] = useState<string>("");
  const [saveStatus, setSaveStatus] = useState<string>("");
  const configLoadSeqRef = useRef(0);

  // --- Computed values ---

  const providerProfileDrafts = useMemo(
    () =>
      Array.isArray(configDraft.provider_profiles)
        ? (configDraft.provider_profiles as ProviderProfileDraft[]).map(
            (entry) => normalizeProviderProfileDraft(entry),
          )
        : [],
    [configDraft.provider_profiles],
  );

  const sectionCardClass =
    appearance === "dark"
      ? "rounded-xl border p-5"
      : "rounded-xl border border-slate-200/80 p-5";
  const sectionCardStyle =
    appearance === "dark"
      ? {
          borderColor:
            "color-mix(in srgb, var(--mc-border-soft) 68%, transparent)",
        }
      : undefined;
  const toggleCardClass =
    appearance === "dark"
      ? "rounded-lg border p-3"
      : "rounded-lg border border-slate-200/80 p-3";
  const toggleCardStyle =
    appearance === "dark"
      ? {
          borderColor:
            "color-mix(in srgb, var(--mc-border-soft) 60%, transparent)",
        }
      : undefined;

  // --- Config field mutators ---

  function setConfigField(field: string, value: unknown): void {
    setConfigDraft((prev) => ({ ...prev, [field]: value }));
  }

  function updateProviderProfile(
    index: number,
    patch: Partial<ProviderProfileDraft>,
  ): void {
    setConfigDraft((prev) => {
      const entries = Array.isArray(prev.provider_profiles)
        ? [...(prev.provider_profiles as ProviderProfileDraft[])]
        : [];
      if (!entries[index]) return prev;
      const oldId = String(entries[index].id || "").trim();
      entries[index] = { ...entries[index], ...patch };
      const nextDraft = { ...prev, provider_profiles: entries };
      if (Object.prototype.hasOwnProperty.call(patch, "id")) {
        return renameProviderProfileReferences(
          nextDraft,
          oldId,
          entries[index].id,
        );
      }
      return nextDraft;
    });
  }

  function addProviderProfile(): void {
    setConfigDraft((prev) => {
      const entries = Array.isArray(prev.provider_profiles)
        ? [...(prev.provider_profiles as ProviderProfileDraft[])]
        : [];
      entries.push(emptyProviderProfileDraft(entries));
      return { ...prev, provider_profiles: entries };
    });
  }

  function cloneProviderProfile(index: number): void {
    setConfigDraft((prev) => {
      const entries = Array.isArray(prev.provider_profiles)
        ? [...(prev.provider_profiles as ProviderProfileDraft[])]
        : [];
      const source = entries[index];
      if (!source) return prev;
      entries.push({
        ...source,
        id: nextClonedProviderProfileId(entries, source.id),
      });
      return { ...prev, provider_profiles: entries };
    });
  }

  function removeProviderProfile(index: number): void {
    setConfigDraft((prev) => {
      const entries = Array.isArray(prev.provider_profiles)
        ? [...(prev.provider_profiles as ProviderProfileDraft[])]
        : [];
      const target = entries[index];
      if (!target) return prev;
      const refs = providerProfileReferences(prev, target.id);
      if (refs.length > 0) return prev;
      entries.splice(index, 1);
      return { ...prev, provider_profiles: entries };
    });
  }

  function resetRefsAndRemoveProviderProfile(index: number): void {
    setConfigDraft((prev) => {
      const entries = Array.isArray(prev.provider_profiles)
        ? [...(prev.provider_profiles as ProviderProfileDraft[])]
        : [];
      const target = entries[index];
      if (!target) return prev;
      const { nextDraft } = resetProviderProfileReferencesToMain(
        prev,
        target.id,
      );
      const nextEntries = Array.isArray(nextDraft.provider_profiles)
        ? [...(nextDraft.provider_profiles as ProviderProfileDraft[])]
        : entries;
      nextEntries.splice(index, 1);
      return { ...nextDraft, provider_profiles: nextEntries };
    });
  }

  function updateA2APeer(index: number, patch: Partial<A2APeerDraft>): void {
    setConfigDraft((prev) => {
      const peers = Array.isArray(prev.a2a_peers)
        ? [...(prev.a2a_peers as A2APeerDraft[])]
        : [];
      if (!peers[index]) return prev;
      peers[index] = { ...peers[index], ...patch };
      return { ...prev, a2a_peers: peers };
    });
  }

  function addA2APeer(): void {
    setConfigDraft((prev) => {
      const peers = Array.isArray(prev.a2a_peers)
        ? [...(prev.a2a_peers as A2APeerDraft[])]
        : [];
      peers.push(emptyA2APeer());
      return { ...prev, a2a_peers: peers };
    });
  }

  function removeA2APeer(index: number): void {
    setConfigDraft((prev) => {
      const peers = Array.isArray(prev.a2a_peers)
        ? [...(prev.a2a_peers as A2APeerDraft[])]
        : [];
      peers.splice(index, 1);
      return { ...prev, a2a_peers: peers };
    });
  }

  function resetConfigField(field: string): void {
    setConfigDraft((prev) => applyConfigFieldReset(prev, field));
  }

  async function openConfig(): Promise<void> {
    const loadSeq = configLoadSeqRef.current + 1;
    configLoadSeqRef.current = loadSeq;
    const isCurrentLoad = () => configLoadSeqRef.current === loadSeq;

    setConfigOpen(true);
    setConfigLoading(true);
    setConfigLoadError("");
    setConfigLoadStage("Loading runtime config...");
    setSaveStatus("");
    setConfig(null);
    setConfigDraft({});
    setSoulFiles([]);
    setConfigSelfCheck(null);
    setConfigSelfCheckError("");
    setConfigSelfCheckLoading(true);

    const selfCheckPromise = api<ConfigSelfCheck>("/api/config/self_check")
      .then((selfCheck) => {
        if (!isCurrentLoad()) return null;
        setConfigSelfCheck(selfCheck);
        return selfCheck;
      })
      .catch((e) => {
        if (!isCurrentLoad()) return null;
        setConfigSelfCheckError(e instanceof Error ? e.message : String(e));
        return null;
      })
      .finally(() => {
        if (isCurrentLoad()) {
          setConfigSelfCheckLoading(false);
        }
      });

    try {
      const data = await api<{ config?: ConfigPayload; soul_files?: string[] }>(
        "/api/config",
      );
      if (!isCurrentLoad()) return;

      setConfigLoadStage("Preparing settings form...");
      setConfig(data.config || null);
      setSoulFiles(
        Array.isArray(data.soul_files)
          ? data.soul_files.map((v) => String(v))
          : [],
      );
      const channelsCfg =
        (data.config?.channels as
          | Record<string, Record<string, unknown>>
          | undefined) || {};
      const telegramCfg = channelsCfg.telegram || {};
      const telegramDefaultAccount =
        defaultAccountIdFromChannelConfig(telegramCfg);
      const telegramAccountCfg = defaultAccountConfig(telegramCfg);
      const telegramAccounts =
        orderedTelegramAccountsFromChannelConfig(telegramCfg);
      const telegramBotCount = normalizeBotCount(telegramAccounts.length || 1);
      const telegramBotDraft: Record<string, unknown> = {};
      for (let slot = 1; slot <= BOT_SLOT_MAX; slot += 1) {
        const account = telegramAccounts[slot - 1];
        const accountId = account?.[0] || defaultTelegramAccountIdForSlot(slot);
        const accountCfg = account?.[1] || {};
        telegramBotDraft[`telegram_bot_${slot}_account_id`] = accountId;
        telegramBotDraft[`telegram_bot_${slot}_token`] = "";
        telegramBotDraft[`telegram_bot_${slot}_has_token`] = Boolean(
          typeof accountCfg.bot_token === "string" &&
          String(accountCfg.bot_token || "").trim(),
        );
        telegramBotDraft[`telegram_bot_${slot}_username`] = String(
          accountCfg.bot_username || "",
        );
        telegramBotDraft[`telegram_bot_${slot}_soul_path`] = String(
          accountCfg.soul_path || "",
        );
        telegramBotDraft[`telegram_bot_${slot}_allowed_user_ids`] =
          Array.isArray(accountCfg.allowed_user_ids)
            ? (accountCfg.allowed_user_ids as number[]).join(",")
            : "";
      }
      const discordCfg = channelsCfg.discord || {};
      const discordDefaultAccount =
        defaultAccountIdFromChannelConfig(discordCfg);
      const discordAccounts = orderedAccountsFromChannelConfig(discordCfg);
      const discordBotCount = normalizeBotCount(discordAccounts.length || 1);
      const discordBotDraft: Record<string, unknown> = {};
      for (let slot = 1; slot <= BOT_SLOT_MAX; slot += 1) {
        const account = discordAccounts[slot - 1];
        const accountId = account?.[0] || defaultAccountIdForSlot(slot);
        const accountCfg = account?.[1] || {};
        discordBotDraft[`discord_bot_${slot}_account_id`] = accountId;
        discordBotDraft[`discord_bot_${slot}_token`] = "";
        discordBotDraft[`discord_bot_${slot}_has_token`] = Boolean(
          typeof accountCfg.bot_token === "string" &&
          String(accountCfg.bot_token || "").trim(),
        );
        discordBotDraft[`discord_bot_${slot}_allowed_channels_csv`] =
          Array.isArray(accountCfg.allowed_channels)
            ? (accountCfg.allowed_channels as number[]).join(",")
            : "";
        discordBotDraft[`discord_bot_${slot}_username`] = String(
          accountCfg.bot_username || "",
        );
        discordBotDraft[`discord_bot_${slot}_provider_preset`] =
          providerPresetFromConfigValue(accountCfg);
      }
      const ircCfg = channelsCfg.irc || {};
      const a2aCfg =
        (data.config?.a2a as Record<string, unknown> | undefined) || {};
      setConfigDraft({
        llm_provider: data.config?.llm_provider || "",
        model:
          data.config?.model ||
          defaultModelForProvider(
            String(data.config?.llm_provider || "anthropic"),
          ),
        llm_base_url: String(data.config?.llm_base_url || ""),
        llm_user_agent: String(data.config?.llm_user_agent || ""),
        api_key: "",
        provider_profiles: providerProfilesFromConfig(data.config || null),
        bot_username: String(data.config?.bot_username || ""),
        telegram_account_id: telegramDefaultAccount,
        telegram_bot_count: telegramBotCount,
        telegram_provider_preset:
          providerPresetFromConfigValue(telegramCfg) ||
          providerPresetFromConfigValue(telegramAccountCfg),
        telegram_allowed_user_ids: Array.isArray(telegramCfg.allowed_user_ids)
          ? (telegramCfg.allowed_user_ids as number[]).join(",")
          : "",
        ...telegramBotDraft,
        discord_account_id: discordDefaultAccount,
        discord_bot_count: discordBotCount,
        discord_provider_preset:
          providerPresetFromConfigValue(discordCfg) ||
          providerPresetFromConfigValue(discordAccounts[0]?.[1] || {}),
        ...discordBotDraft,
        irc_server: String(ircCfg.server || ""),
        irc_port: String(ircCfg.port || ""),
        irc_nick: String(ircCfg.nick || ""),
        irc_username: String(ircCfg.username || ""),
        irc_real_name: String(ircCfg.real_name || ""),
        irc_channels: String(ircCfg.channels || ""),
        irc_password: "",
        irc_mention_required: String(ircCfg.mention_required || ""),
        irc_tls: String(ircCfg.tls || ""),
        irc_tls_server_name: String(ircCfg.tls_server_name || ""),
        irc_tls_danger_accept_invalid_certs: String(
          ircCfg.tls_danger_accept_invalid_certs || "",
        ),
        irc_provider_preset: providerPresetFromConfigValue(ircCfg),
        web_bot_username: String(channelsCfg.web?.bot_username || ""),
        working_dir_isolation: normalizeWorkingDirIsolation(
          data.config?.working_dir_isolation ||
            DEFAULT_CONFIG_VALUES.working_dir_isolation,
        ),
        high_risk_tool_user_confirmation_required:
          data.config?.high_risk_tool_user_confirmation_required !== false,
        max_tokens: Number(data.config?.max_tokens ?? 8192),
        max_tool_iterations: Number(data.config?.max_tool_iterations ?? 100),
        max_document_size_mb: Number(
          data.config?.max_document_size_mb ??
            DEFAULT_CONFIG_VALUES.max_document_size_mb,
        ),
        memory_token_budget: Number(
          data.config?.memory_token_budget ??
            DEFAULT_CONFIG_VALUES.memory_token_budget,
        ),
        show_thinking: Boolean(data.config?.show_thinking),
        web_enabled: Boolean(data.config?.web_enabled),
        web_host: String(data.config?.web_host || "127.0.0.1"),
        web_port: Number(data.config?.web_port ?? 10961),
        reflector_enabled: data.config?.reflector_enabled !== false,
        reflector_interval_mins: Number(
          data.config?.reflector_interval_mins ??
            DEFAULT_CONFIG_VALUES.reflector_interval_mins,
        ),
        embedding_provider: String(data.config?.embedding_provider || ""),
        embedding_api_key: "",
        embedding_base_url: String(data.config?.embedding_base_url || ""),
        embedding_model: String(data.config?.embedding_model || ""),
        embedding_dim: String(data.config?.embedding_dim || ""),
        a2a_enabled: Boolean(a2aCfg.enabled),
        a2a_public_base_url: String(a2aCfg.public_base_url || ""),
        a2a_agent_name: String(a2aCfg.agent_name || ""),
        a2a_agent_description: String(a2aCfg.agent_description || ""),
        a2a_shared_tokens: "",
        a2a_peers: peersFromConfigValue(a2aCfg.peers),
        souls_dir: String(
          data.config?.souls_dir ||
            (String(data.config?.data_dir || "").trim()
              ? `${String(data.config?.data_dir).trim()}/souls`
              : ""),
        ),
        // Dynamic channel fields — initialize from server config
        ...Object.fromEntries(
          DYNAMIC_CHANNELS.flatMap((ch) => {
            const chCfg = channelsCfg[ch.name] || {};
            const chAccounts = orderedAccountsFromChannelConfig(chCfg);
            const botCount = normalizeBotCount(chAccounts.length || 1);
            const pairs: Array<[string, unknown]> = [
              [
                `${ch.name}__account_id`,
                defaultAccountIdFromChannelConfig(chCfg),
              ],
              [`${ch.name}__bot_count`, botCount],
            ];
            for (const f of ch.channelFields || []) {
              if (f.secret) {
                pairs.push([
                  `${ch.name}__has__${f.yamlKey}`,
                  Boolean(String(chCfg[f.yamlKey] || "").trim()),
                ]);
                pairs.push([`${ch.name}__${f.yamlKey}`, ""]);
              } else {
                pairs.push([
                  `${ch.name}__${f.yamlKey}`,
                  dynamicFieldDraftValue(
                    chCfg[f.yamlKey],
                    f.valueType || "string",
                  ),
                ]);
              }
            }
            for (let slot = 1; slot <= BOT_SLOT_MAX; slot += 1) {
              const account = chAccounts[slot - 1];
              const accountId = account?.[0] || defaultAccountIdForSlot(slot);
              const accountCfg = account?.[1] || {};
              pairs.push([`${ch.name}__bot_${slot}__account_id`, accountId]);
              pairs.push([
                `${ch.name}__bot_${slot}__soul_path`,
                String(accountCfg.soul_path || ""),
              ]);
              for (const f of ch.fields) {
                if (f.secret) {
                  pairs.push([
                    `${ch.name}__bot_${slot}__has__${f.yamlKey}`,
                    Boolean(String(accountCfg[f.yamlKey] || "").trim()),
                  ]);
                  pairs.push([`${ch.name}__bot_${slot}__${f.yamlKey}`, ""]);
                } else {
                  const value =
                    f.yamlKey === "provider_preset"
                      ? providerPresetFromConfigValue(accountCfg)
                      : dynamicFieldDraftValue(
                          accountCfg[f.yamlKey],
                          f.valueType || "string",
                        );
                  pairs.push([`${ch.name}__bot_${slot}__${f.yamlKey}`, value]);
                }
              }
            }
            return pairs;
          }),
        ),
      });
      setConfigLoadStage("Finishing checks...");
      void selfCheckPromise;
    } catch (e) {
      if (!isCurrentLoad()) return;
      if (isUnauthorizedError(e)) {
        lockForAuth("Session expired. Please sign in again.");
        setConfigOpen(false);
        return;
      }
      if (isForbiddenError(e)) {
        setConfigLoadError(
          "Forbidden: Runtime Config is not accessible with current credentials.",
        );
        return;
      }
      setConfigLoadError(e instanceof Error ? e.message : String(e));
    } finally {
      if (isCurrentLoad()) {
        setConfigLoading(false);
        setConfigLoadStage("");
      }
    }
  }

  async function saveConfigChanges(): Promise<void> {
    try {
      const provider = String(configDraft.llm_provider || "")
        .trim()
        .toLowerCase();
      if (provider === "openai-codex") {
        const apiKey = String(configDraft.api_key || "").trim();
        const baseUrl = String(configDraft.llm_base_url || "").trim();
        if (apiKey || baseUrl) {
          setSaveStatus(
            "Save failed: openai-codex ignores api_key/llm_base_url in microclaw config. Configure ~/.codex/auth.json and ~/.codex/config.toml.",
          );
          return;
        }
      }

      const payload = buildConfigPayload(configDraft, providerProfileDrafts);

      await api("/api/config", {
        method: "PUT",
        body: JSON.stringify(payload),
      });
      setConfigSelfCheckLoading(true);
      setConfigSelfCheckError("");
      const selfCheck = await api<ConfigSelfCheck>(
        "/api/config/self_check",
      ).catch((e) => {
        setConfigSelfCheckError(e instanceof Error ? e.message : String(e));
        return null;
      });
      setConfigSelfCheck(selfCheck);
      setConfigSelfCheckLoading(false);
      setSaveStatus("Saved. Restart microclaw to apply changes.");
    } catch (e) {
      setSaveStatus(
        `Save failed: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
  }

  return {
    // state
    configOpen,
    setConfigOpen,
    configLoading,
    configLoadStage,
    configLoadError,
    config,
    configDraft,
    soulFiles,
    configSelfCheck,
    configSelfCheckLoading,
    configSelfCheckError,
    saveStatus,
    // computed
    providerProfileDrafts,
    sectionCardClass,
    sectionCardStyle,
    toggleCardClass,
    toggleCardStyle,
    // functions
    setConfigField,
    updateProviderProfile,
    addProviderProfile,
    cloneProviderProfile,
    removeProviderProfile,
    resetRefsAndRemoveProviderProfile,
    updateA2APeer,
    addA2APeer,
    removeA2APeer,
    resetConfigField,
    openConfig,
    saveConfigChanges,
  };
}
