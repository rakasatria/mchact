/**
 * Pure function that builds the config payload for saving.
 * Takes the config draft and provider profile drafts,
 * returns the serialized payload object ready for the API.
 * Throws on validation failures (duplicate account IDs, etc.).
 */

import type { A2APeerDraft, ProviderProfileDraft } from "./types";
import { DEFAULT_CONFIG_VALUES, BOT_SLOT_MAX } from "./constants";
import { DYNAMIC_CHANNELS } from "./channels";

import {
  normalizeAccountId,
  normalizeBotCount,
  normalizeSoulPathInput,
  normalizeWorkingDirIsolation,
  parseDiscordChannelCsv,
  parseI64ListCsvOrJsonArray,
  parseStringListInput,
  parseOptionalBoolString,
  parseOptionalU64String,
  defaultTelegramAccountIdForSlot,
  defaultAccountIdForSlot,
} from "./config-helpers";

import { serializeProviderProfiles } from "./provider-profiles";

export function buildConfigPayload(
  configDraft: Record<string, unknown>,
  providerProfileDrafts: ProviderProfileDraft[],
): Record<string, unknown> {
  const provider = String(configDraft.llm_provider || "")
    .trim()
    .toLowerCase();

  const payload: Record<string, unknown> = {
    llm_provider: String(configDraft.llm_provider || ""),
    model: String(configDraft.model || ""),
    llm_user_agent: String(configDraft.llm_user_agent || "").trim() || null,
    provider_presets: serializeProviderProfiles(providerProfileDrafts),
    bot_username: String(configDraft.bot_username || "").trim(),
    web_bot_username:
      String(configDraft.web_bot_username || "").trim() || null,
    working_dir_isolation: normalizeWorkingDirIsolation(
      configDraft.working_dir_isolation ||
        DEFAULT_CONFIG_VALUES.working_dir_isolation,
    ),
    high_risk_tool_user_confirmation_required:
      configDraft.high_risk_tool_user_confirmation_required !== false,
    max_tokens: Number(configDraft.max_tokens || 8192),
    max_tool_iterations: Number(configDraft.max_tool_iterations || 100),
    max_document_size_mb: Number(
      configDraft.max_document_size_mb ||
        DEFAULT_CONFIG_VALUES.max_document_size_mb,
    ),
    memory_token_budget: Number(
      configDraft.memory_token_budget ||
        DEFAULT_CONFIG_VALUES.memory_token_budget,
    ),
    show_thinking: Boolean(configDraft.show_thinking),
    web_enabled: Boolean(configDraft.web_enabled),
    web_host: String(configDraft.web_host || "127.0.0.1"),
    web_port: Number(configDraft.web_port || 10961),
    reflector_enabled: configDraft.reflector_enabled !== false,
    reflector_interval_mins: Number(
      configDraft.reflector_interval_mins ||
        DEFAULT_CONFIG_VALUES.reflector_interval_mins,
    ),
    embedding_provider:
      String(configDraft.embedding_provider || "").trim() || null,
    embedding_base_url:
      String(configDraft.embedding_base_url || "").trim() || null,
    embedding_model:
      String(configDraft.embedding_model || "").trim() || null,
    embedding_dim: String(configDraft.embedding_dim || "").trim()
      ? Number(configDraft.embedding_dim)
      : null,
    a2a_enabled: Boolean(configDraft.a2a_enabled),
    a2a_public_base_url:
      String(configDraft.a2a_public_base_url || "").trim() || null,
    a2a_agent_name: String(configDraft.a2a_agent_name || "").trim() || null,
    a2a_agent_description:
      String(configDraft.a2a_agent_description || "").trim() || null,
    souls_dir: String(configDraft.souls_dir || "").trim() || null,
  };

  // LLM base URL
  if (
    String(configDraft.llm_provider || "")
      .trim()
      .toLowerCase() === "custom"
  ) {
    payload.llm_base_url =
      String(configDraft.llm_base_url || "").trim() || null;
  } else if (provider === "openai-codex") {
    payload.llm_base_url = null;
  }

  // API key
  const apiKey = String(configDraft.api_key || "").trim();
  if (provider === "openai-codex") {
    payload.api_key = "";
  } else if (apiKey) {
    payload.api_key = apiKey;
  }

  // --- Telegram ---
  const telegramAccountId = normalizeAccountId(
    configDraft.telegram_account_id,
  );
  const telegramProviderPreset = String(
    configDraft.telegram_provider_preset || "",
  ).trim();
  const telegramBotCount = normalizeBotCount(
    configDraft.telegram_bot_count,
  );
  const telegramAllowedUserIds = parseI64ListCsvOrJsonArray(
    String(configDraft.telegram_allowed_user_ids || ""),
    "telegram_allowed_user_ids",
  );
  const telegramAccounts: Record<string, unknown> = {};
  for (let slot = 1; slot <= telegramBotCount; slot += 1) {
    const accountId = normalizeAccountId(
      configDraft[`telegram_bot_${slot}_account_id`] ||
        defaultTelegramAccountIdForSlot(slot),
    );
    const token = String(
      configDraft[`telegram_bot_${slot}_token`] || "",
    ).trim();
    const hasToken = Boolean(configDraft[`telegram_bot_${slot}_has_token`]);
    const username = String(
      configDraft[`telegram_bot_${slot}_username`] || "",
    ).trim();
    const providerPreset = String(
      configDraft[`telegram_bot_${slot}_provider_preset`] || "",
    ).trim();
    const soulPath = normalizeSoulPathInput(
      configDraft[`telegram_bot_${slot}_soul_path`],
      configDraft.souls_dir,
    );
    const accountAllowedUserIds = parseI64ListCsvOrJsonArray(
      String(configDraft[`telegram_bot_${slot}_allowed_user_ids`] || ""),
      `telegram_bot_${slot}_allowed_user_ids`,
    );
    const hasAny =
      Boolean(token) ||
      hasToken ||
      Boolean(username) ||
      Boolean(providerPreset) ||
      Boolean(soulPath) ||
      accountAllowedUserIds.length > 0 ||
      accountId === telegramAccountId;
    if (!hasAny) continue;
    if (Object.prototype.hasOwnProperty.call(telegramAccounts, accountId)) {
      throw new Error(`Duplicate Telegram account id: ${accountId}`);
    }
    telegramAccounts[accountId] = {
      enabled: true,
      ...(token ? { bot_token: token } : {}),
      ...(username ? { bot_username: username } : {}),
      ...(providerPreset ? { provider_preset: providerPreset } : {}),
      ...(soulPath ? { soul_path: soulPath } : {}),
      ...(accountAllowedUserIds.length > 0
        ? { allowed_user_ids: accountAllowedUserIds }
        : {}),
    };
  }

  // --- Discord ---
  const discordAccountId = normalizeAccountId(
    configDraft.discord_account_id,
  );
  const discordProviderPreset = String(
    configDraft.discord_provider_preset || "",
  ).trim();
  const discordBotCount = normalizeBotCount(configDraft.discord_bot_count);
  const discordAccounts: Record<string, unknown> = {};
  for (let slot = 1; slot <= discordBotCount; slot += 1) {
    const accountId = normalizeAccountId(
      configDraft[`discord_bot_${slot}_account_id`] ||
        defaultAccountIdForSlot(slot),
    );
    const token = String(
      configDraft[`discord_bot_${slot}_token`] || "",
    ).trim();
    const hasToken = Boolean(configDraft[`discord_bot_${slot}_has_token`]);
    const allowedChannels = parseDiscordChannelCsv(
      String(configDraft[`discord_bot_${slot}_allowed_channels_csv`] || ""),
    );
    const username = String(
      configDraft[`discord_bot_${slot}_username`] || "",
    ).trim();
    const providerPreset = String(
      configDraft[`discord_bot_${slot}_provider_preset`] || "",
    ).trim();
    const hasAny =
      Boolean(token) ||
      hasToken ||
      allowedChannels.length > 0 ||
      Boolean(username) ||
      Boolean(providerPreset) ||
      accountId === discordAccountId;
    if (!hasAny) continue;
    if (Object.prototype.hasOwnProperty.call(discordAccounts, accountId)) {
      throw new Error(`Duplicate Discord account id: ${accountId}`);
    }
    discordAccounts[accountId] = {
      enabled: true,
      ...(token ? { bot_token: token } : {}),
      ...(allowedChannels.length > 0
        ? { allowed_channels: allowedChannels }
        : {}),
      ...(username ? { bot_username: username } : {}),
      ...(providerPreset ? { provider_preset: providerPreset } : {}),
    };
  }

  // --- IRC ---
  const ircServer = String(configDraft.irc_server || "").trim();
  const ircPort = String(configDraft.irc_port || "").trim();
  const ircNick = String(configDraft.irc_nick || "").trim();
  const ircUsername = String(configDraft.irc_username || "").trim();
  const ircRealName = String(configDraft.irc_real_name || "").trim();
  const ircChannels = String(configDraft.irc_channels || "").trim();
  const ircPassword = String(configDraft.irc_password || "").trim();
  const ircMentionRequired = String(
    configDraft.irc_mention_required || "",
  ).trim();
  const ircTls = String(configDraft.irc_tls || "").trim();
  const ircTlsServerName = String(
    configDraft.irc_tls_server_name || "",
  ).trim();
  const ircTlsDangerAcceptInvalidCerts = String(
    configDraft.irc_tls_danger_accept_invalid_certs || "",
  ).trim();
  const ircProviderPreset = String(
    configDraft.irc_provider_preset || "",
  ).trim();

  // --- Embedding API key ---
  const embeddingApiKey = String(
    configDraft.embedding_api_key || "",
  ).trim();
  if (embeddingApiKey) payload.embedding_api_key = embeddingApiKey;

  // --- A2A shared tokens ---
  const a2aSharedTokens = String(
    configDraft.a2a_shared_tokens || "",
  ).trim();
  if (a2aSharedTokens) {
    payload.a2a_shared_tokens = parseStringListInput(a2aSharedTokens);
  }

  // --- A2A peers ---
  const a2aPeers = Array.isArray(configDraft.a2a_peers)
    ? (configDraft.a2a_peers as A2APeerDraft[])
    : [];
  if (a2aPeers.length > 0) {
    const serializedPeers: Record<string, unknown> = {};
    for (const [index, peer] of a2aPeers.entries()) {
      const name = String(peer.name || "").trim();
      const baseUrl = String(peer.base_url || "").trim();
      const bearerToken = String(peer.bearer_token || "").trim();
      const hasBearerToken = Boolean(peer.has_bearer_token);
      const description = String(peer.description || "").trim();
      const defaultSessionKey = String(
        peer.default_session_key || "",
      ).trim();
      if (
        !name &&
        !baseUrl &&
        !bearerToken &&
        !description &&
        !defaultSessionKey &&
        !hasBearerToken
      ) {
        continue;
      }
      if (!name) {
        throw new Error(`A2A peer #${index + 1} is missing a name`);
      }
      if (Object.prototype.hasOwnProperty.call(serializedPeers, name)) {
        throw new Error(`Duplicate A2A peer name: ${name}`);
      }
      serializedPeers[name] = {
        enabled: peer.enabled !== false,
        ...(baseUrl ? { base_url: baseUrl } : {}),
        ...(bearerToken ? { bearer_token: bearerToken } : {}),
        ...(description ? { description } : {}),
        ...(defaultSessionKey
          ? { default_session_key: defaultSessionKey }
          : {}),
      };
    }
    if (Object.keys(serializedPeers).length > 0)
      payload.a2a_peers = serializedPeers;
  }

  // --- Build channel configs ---
  const channelConfigs: Record<string, Record<string, unknown>> = {};
  if (
    Object.keys(telegramAccounts).length > 0 ||
    telegramAllowedUserIds.length > 0 ||
    telegramProviderPreset
  ) {
    channelConfigs.telegram = {
      default_account: telegramAccountId,
      ...(telegramProviderPreset
        ? { provider_preset: telegramProviderPreset }
        : {}),
      ...(telegramAllowedUserIds.length > 0
        ? { allowed_user_ids: telegramAllowedUserIds }
        : {}),
      accounts: telegramAccounts,
    };
  }
  if (Object.keys(discordAccounts).length > 0) {
    channelConfigs.discord = {
      default_account: discordAccountId,
      ...(discordProviderPreset
        ? { provider_preset: discordProviderPreset }
        : {}),
      accounts: discordAccounts,
    };
  }
  if (
    ircServer ||
    ircPort ||
    ircNick ||
    ircUsername ||
    ircRealName ||
    ircChannels ||
    ircPassword ||
    ircMentionRequired ||
    ircTls ||
    ircTlsServerName ||
    ircTlsDangerAcceptInvalidCerts ||
    ircProviderPreset
  ) {
    channelConfigs.irc = {
      ...(ircServer ? { server: ircServer } : {}),
      ...(ircPort ? { port: ircPort } : {}),
      ...(ircNick ? { nick: ircNick } : {}),
      ...(ircUsername ? { username: ircUsername } : {}),
      ...(ircRealName ? { real_name: ircRealName } : {}),
      ...(ircChannels ? { channels: ircChannels } : {}),
      ...(ircPassword ? { password: ircPassword } : {}),
      ...(ircMentionRequired
        ? { mention_required: ircMentionRequired }
        : {}),
      ...(ircTls ? { tls: ircTls } : {}),
      ...(ircTlsServerName ? { tls_server_name: ircTlsServerName } : {}),
      ...(ircTlsDangerAcceptInvalidCerts
        ? {
            tls_danger_accept_invalid_certs: ircTlsDangerAcceptInvalidCerts,
          }
        : {}),
      ...(ircProviderPreset ? { provider_preset: ircProviderPreset } : {}),
    };
  }

  // --- Dynamic channels ---
  for (const ch of DYNAMIC_CHANNELS) {
    const accountId = normalizeAccountId(
      configDraft[`${ch.name}__account_id`],
    );
    const botCount = normalizeBotCount(
      configDraft[`${ch.name}__bot_count`],
    );
    const accounts: Record<string, unknown> = {};
    const channelFields: Record<string, unknown> = {};
    for (const f of ch.channelFields || []) {
      const key = `${ch.name}__${f.yamlKey}`;
      const val = String(configDraft[key] || "").trim();
      if (!val) continue;
      if ((f.valueType || "string") === "bool") {
        const parsed = parseOptionalBoolString(
          val,
          `${ch.name}_${f.yamlKey}`,
        );
        if (parsed !== null) {
          channelFields[f.yamlKey] = parsed;
        }
      } else if ((f.valueType || "string") === "number") {
        const parsed = parseOptionalU64String(
          val,
          `${ch.name}_${f.yamlKey}`,
        );
        if (parsed !== null) {
          channelFields[f.yamlKey] = parsed;
        }
      } else {
        channelFields[f.yamlKey] = val;
      }
    }
    for (let slot = 1; slot <= botCount; slot += 1) {
      const slotAccountId = normalizeAccountId(
        configDraft[`${ch.name}__bot_${slot}__account_id`] ||
          defaultAccountIdForSlot(slot),
      );
      const soulPath = normalizeSoulPathInput(
        configDraft[`${ch.name}__bot_${slot}__soul_path`],
        configDraft.souls_dir,
      );
      const fields: Record<string, unknown> = {};
      let hasAny = slotAccountId === accountId || Boolean(soulPath);
      for (const f of ch.fields) {
        const key = `${ch.name}__bot_${slot}__${f.yamlKey}`;
        const val = String(configDraft[key] || "").trim();
        const hasSecret = f.secret
          ? Boolean(
              configDraft[`${ch.name}__bot_${slot}__has__${f.yamlKey}`],
            )
          : false;
        if (val) {
          if ((f.valueType || "string") === "bool") {
            const parsed = parseOptionalBoolString(
              val,
              `${ch.name}_bot_${slot}_${f.yamlKey}`,
            );
            if (parsed !== null) {
              fields[f.yamlKey] = parsed;
            }
          } else if ((f.valueType || "string") === "number") {
            const parsed = parseOptionalU64String(
              val,
              `${ch.name}_bot_${slot}_${f.yamlKey}`,
            );
            if (parsed !== null) {
              fields[f.yamlKey] = parsed;
            }
          } else {
            fields[f.yamlKey] = val;
          }
          hasAny = true;
        } else if (hasSecret) {
          hasAny = true;
        }
      }
      if (!hasAny) continue;
      if (Object.prototype.hasOwnProperty.call(accounts, slotAccountId)) {
        throw new Error(
          `Duplicate ${ch.name} account id: ${slotAccountId}`,
        );
      }
      accounts[slotAccountId] = {
        enabled: true,
        ...(soulPath ? { soul_path: soulPath } : {}),
        ...fields,
      };
    }
    if (
      Object.keys(accounts).length > 0 ||
      Object.keys(channelFields).length > 0
    ) {
      channelConfigs[ch.name] = {
        ...channelFields,
        ...(Object.keys(accounts).length > 0
          ? {
              default_account: accountId,
              accounts: {
                ...accounts,
              },
            }
          : {}),
      };
    }
  }
  if (Object.keys(channelConfigs).length > 0) {
    payload.channel_configs = channelConfigs;
  }

  return payload;
}
