/**
 * Pure function that applies a config field reset to a draft object.
 * Returns a new draft with the specified field reset to its default value.
 */

import { DEFAULT_CONFIG_VALUES, BOT_SLOT_MAX } from "./constants";
import { DYNAMIC_CHANNELS } from "./channels";
import {
  defaultModelForProvider,
  defaultTelegramAccountIdForSlot,
  defaultAccountIdForSlot,
} from "./config-helpers";

export function applyConfigFieldReset(
  draft: Record<string, unknown>,
  field: string,
): Record<string, unknown> {
  const next = { ...draft };
  switch (field) {
    case "llm_provider":
      next.llm_provider = DEFAULT_CONFIG_VALUES.llm_provider;
      next.model = defaultModelForProvider(
        DEFAULT_CONFIG_VALUES.llm_provider,
      );
      break;
    case "model":
      next.model = defaultModelForProvider(
        String(next.llm_provider || DEFAULT_CONFIG_VALUES.llm_provider),
      );
      break;
    case "llm_base_url":
      next.llm_base_url = "";
      break;
    case "llm_user_agent":
      next.llm_user_agent = "";
      break;
    case "max_tokens":
      next.max_tokens = DEFAULT_CONFIG_VALUES.max_tokens;
      break;
    case "telegram_account_id":
      next.telegram_account_id = "main";
      break;
    case "telegram_bot_count":
      next.telegram_bot_count = 1;
      for (let slot = 1; slot <= BOT_SLOT_MAX; slot += 1) {
        next[`telegram_bot_${slot}_account_id`] =
          defaultTelegramAccountIdForSlot(slot);
        next[`telegram_bot_${slot}_token`] = "";
        next[`telegram_bot_${slot}_has_token`] = false;
        next[`telegram_bot_${slot}_username`] = "";
        next[`telegram_bot_${slot}_soul_path`] = "";
        next[`telegram_bot_${slot}_allowed_user_ids`] = "";
      }
      break;
    case "bot_username":
      next.bot_username = "";
      break;
    case "telegram_provider_preset":
      next.telegram_provider_preset = "";
      break;
    case "telegram_allowed_user_ids":
      next.telegram_allowed_user_ids = "";
      break;
    case "discord_account_id":
      next.discord_account_id = "main";
      break;
    case "discord_provider_preset":
      next.discord_provider_preset = "";
      break;
    case "discord_bot_count":
      next.discord_bot_count = 1;
      for (let slot = 1; slot <= BOT_SLOT_MAX; slot += 1) {
        next[`discord_bot_${slot}_account_id`] =
          defaultAccountIdForSlot(slot);
        next[`discord_bot_${slot}_token`] = "";
        next[`discord_bot_${slot}_has_token`] = false;
        next[`discord_bot_${slot}_allowed_channels_csv`] = "";
        next[`discord_bot_${slot}_username`] = "";
        next[`discord_bot_${slot}_provider_preset`] = "";
      }
      break;
    case "provider_profiles":
      next.provider_profiles = [];
      break;
    case "web_bot_username":
      next.web_bot_username = "";
      break;
    case "working_dir_isolation":
      next.working_dir_isolation =
        DEFAULT_CONFIG_VALUES.working_dir_isolation;
      break;
    case "high_risk_tool_user_confirmation_required":
      next.high_risk_tool_user_confirmation_required =
        DEFAULT_CONFIG_VALUES.high_risk_tool_user_confirmation_required;
      break;
    case "max_tool_iterations":
      next.max_tool_iterations = DEFAULT_CONFIG_VALUES.max_tool_iterations;
      break;
    case "max_document_size_mb":
      next.max_document_size_mb =
        DEFAULT_CONFIG_VALUES.max_document_size_mb;
      break;
    case "memory_token_budget":
      next.memory_token_budget = DEFAULT_CONFIG_VALUES.memory_token_budget;
      break;
    case "show_thinking":
      next.show_thinking = DEFAULT_CONFIG_VALUES.show_thinking;
      break;
    case "web_enabled":
      next.web_enabled = DEFAULT_CONFIG_VALUES.web_enabled;
      break;
    case "web_host":
      next.web_host = DEFAULT_CONFIG_VALUES.web_host;
      break;
    case "web_port":
      next.web_port = DEFAULT_CONFIG_VALUES.web_port;
      break;
    case "reflector_enabled":
      next.reflector_enabled = DEFAULT_CONFIG_VALUES.reflector_enabled;
      break;
    case "reflector_interval_mins":
      next.reflector_interval_mins =
        DEFAULT_CONFIG_VALUES.reflector_interval_mins;
      break;
    case "embedding_provider":
      next.embedding_provider = DEFAULT_CONFIG_VALUES.embedding_provider;
      break;
    case "embedding_api_key":
      next.embedding_api_key = DEFAULT_CONFIG_VALUES.embedding_api_key;
      break;
    case "embedding_base_url":
      next.embedding_base_url = DEFAULT_CONFIG_VALUES.embedding_base_url;
      break;
    case "embedding_model":
      next.embedding_model = DEFAULT_CONFIG_VALUES.embedding_model;
      break;
    case "embedding_dim":
      next.embedding_dim = DEFAULT_CONFIG_VALUES.embedding_dim;
      break;
    case "a2a_enabled":
      next.a2a_enabled = DEFAULT_CONFIG_VALUES.a2a_enabled;
      break;
    case "a2a_public_base_url":
      next.a2a_public_base_url = DEFAULT_CONFIG_VALUES.a2a_public_base_url;
      break;
    case "a2a_agent_name":
      next.a2a_agent_name = DEFAULT_CONFIG_VALUES.a2a_agent_name;
      break;
    case "a2a_agent_description":
      next.a2a_agent_description =
        DEFAULT_CONFIG_VALUES.a2a_agent_description;
      break;
    case "a2a_shared_tokens":
      next.a2a_shared_tokens = DEFAULT_CONFIG_VALUES.a2a_shared_tokens;
      break;
    case "a2a_peers":
      next.a2a_peers = DEFAULT_CONFIG_VALUES.a2a_peers;
      break;
    case "irc_server":
      next.irc_server = "";
      break;
    case "irc_port":
      next.irc_port = "";
      break;
    case "irc_nick":
      next.irc_nick = "";
      break;
    case "irc_username":
      next.irc_username = "";
      break;
    case "irc_real_name":
      next.irc_real_name = "";
      break;
    case "irc_channels":
      next.irc_channels = "";
      break;
    case "irc_password":
      next.irc_password = "";
      break;
    case "irc_mention_required":
      next.irc_mention_required = "";
      break;
    case "irc_tls":
      next.irc_tls = "";
      break;
    case "irc_tls_server_name":
      next.irc_tls_server_name = "";
      break;
    case "irc_tls_danger_accept_invalid_certs":
      next.irc_tls_danger_accept_invalid_certs = "";
      break;
    case "irc_provider_preset":
      next.irc_provider_preset = "";
      break;
    default:
      for (let slot = 1; slot <= BOT_SLOT_MAX; slot += 1) {
        if (field === `telegram_bot_${slot}_soul_path`) {
          next[`telegram_bot_${slot}_soul_path`] = "";
        }
      }
      // Handle dynamic channel fields
      for (const ch of DYNAMIC_CHANNELS) {
        const accountKey = `${ch.name}__account_id`;
        const botCountKey = `${ch.name}__bot_count`;
        if (field === accountKey) {
          next[accountKey] = "main";
        }
        if (field === botCountKey) {
          next[botCountKey] = 1;
          for (const f of ch.channelFields || []) {
            next[`${ch.name}__${f.yamlKey}`] = "";
            if (f.secret) next[`${ch.name}__has__${f.yamlKey}`] = false;
          }
          for (let slot = 1; slot <= BOT_SLOT_MAX; slot += 1) {
            next[`${ch.name}__bot_${slot}__account_id`] =
              defaultAccountIdForSlot(slot);
            next[`${ch.name}__bot_${slot}__soul_path`] = "";
            for (const f of ch.fields) {
              next[`${ch.name}__bot_${slot}__${f.yamlKey}`] = "";
              if (f.secret)
                next[`${ch.name}__bot_${slot}__has__${f.yamlKey}`] = false;
            }
          }
        }
        for (let slot = 1; slot <= BOT_SLOT_MAX; slot += 1) {
          if (field === `${ch.name}__bot_${slot}__soul_path`) {
            next[`${ch.name}__bot_${slot}__soul_path`] = "";
          }
        }
        for (const f of ch.channelFields || []) {
          const key = `${ch.name}__${f.yamlKey}`;
          if (field === key) {
            next[key] = "";
            if (f.secret) next[`${ch.name}__has__${f.yamlKey}`] = false;
          }
        }
        for (const f of ch.fields) {
          for (let slot = 1; slot <= BOT_SLOT_MAX; slot += 1) {
            const key = `${ch.name}__bot_${slot}__${f.yamlKey}`;
            if (field === key) {
              next[key] = "";
              if (f.secret)
                next[`${ch.name}__bot_${slot}__has__${f.yamlKey}`] = false;
            }
          }
        }
      }
      break;
  }
  return next;
}
