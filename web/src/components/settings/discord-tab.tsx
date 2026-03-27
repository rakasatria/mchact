import React from "react";
import { Card, Select, Text, TextField } from "@radix-ui/themes";
import { ConfigFieldCard } from "../config-field-card";
import { ConfigStepsCard } from "../config-steps-card";
import { useSettings } from "../../context/settings-context";
import { BOT_SLOT_MAX, MAIN_PROFILE_VALUE } from "../../lib/constants";
import {
  normalizeBotCount,
  defaultAccountIdForSlot,
} from "../../lib/config-helpers";
import { providerProfileOptions } from "../../lib/provider-profiles";

export function DiscordTab(): React.ReactElement {
  const {
    configDraft,
    setConfigField,
    providerProfileDrafts,
    sectionCardClass,
    sectionCardStyle,
  } = useSettings();

  return (
    <div
      className={sectionCardClass}
      style={sectionCardStyle}
    >
      <Text size="3" weight="bold">
        Discord
      </Text>
      <ConfigStepsCard
        steps={[
          <>
            Open Discord Developer Portal and create an
            application + bot.
          </>,
          <>
            Enable <code>Message Content Intent</code> under
            Bot settings.
          </>,
          <>
            Invite bot with scopes/permissions: bot, View
            Channels, Send Messages, Read Message History.
          </>,
          <>Paste bot token below.</>,
          <>
            Optional: limit handling to specific channel
            IDs.
          </>,
        ]}
      />
      <Text size="1" color="gray" className="mt-3 block">
        Configure one or more Discord bot accounts (up to
        10).
      </Text>
      <div className="mt-4 space-y-3">
        <ConfigFieldCard
          label="discord_default_account"
          description={
            <>
              Default account id under{" "}
              <code>channels.discord.accounts</code>.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(
              configDraft.discord_account_id || "main",
            )}
            onChange={(e) =>
              setConfigField(
                "discord_account_id",
                e.target.value,
              )
            }
            placeholder="main"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="discord_bot_count"
          description={
            <>
              Number of Discord bot accounts to configure
              (1-10).
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            type="number"
            min="1"
            max={String(BOT_SLOT_MAX)}
            value={String(
              configDraft.discord_bot_count || 1,
            )}
            onChange={(e) =>
              setConfigField(
                "discord_bot_count",
                normalizeBotCount(e.target.value),
              )
            }
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="discord_provider_preset"
          description={
            <>
              Optional Discord channel-level LLM provider
              profile override.
            </>
          }
        >
          <div className="mt-2">
            <Select.Root
              value={
                String(
                  configDraft.discord_provider_preset || "",
                ) || MAIN_PROFILE_VALUE
              }
              onValueChange={(value) =>
                setConfigField(
                  "discord_provider_preset",
                  value === MAIN_PROFILE_VALUE ? "" : value,
                )
              }
            >
              <Select.Trigger
                className="w-full mc-select-trigger-full"
                placeholder="Select provider profile"
              />
              <Select.Content>
                {providerProfileOptions(
                  providerProfileDrafts,
                  configDraft.discord_provider_preset,
                ).map((option) => (
                  <Select.Item
                    key={`discord-provider-preset-${option.value}`}
                    value={option.value}
                  >
                    {option.label}
                  </Select.Item>
                ))}
              </Select.Content>
            </Select.Root>
          </div>
        </ConfigFieldCard>
        {Array.from({
          length: normalizeBotCount(
            configDraft.discord_bot_count || 1,
          ),
        }).map((_, idx) => {
          const slot = idx + 1;
          return (
            <Card
              key={`discord-bot-${slot}`}
              className="p-3"
            >
              <Text size="2" weight="medium">
                Discord bot #{slot}
              </Text>
              <div className="mt-2 space-y-3">
                <ConfigFieldCard
                  label={`discord_bot_${slot}_account_id`}
                  description={
                    <>
                      Bot account id used under{" "}
                      <code>channels.discord.accounts</code>
                      .
                    </>
                  }
                >
                  <TextField.Root
                    className="mt-2"
                    value={String(
                      configDraft[
                        `discord_bot_${slot}_account_id`
                      ] || defaultAccountIdForSlot(slot),
                    )}
                    onChange={(e) =>
                      setConfigField(
                        `discord_bot_${slot}_account_id`,
                        e.target.value,
                      )
                    }
                    placeholder={defaultAccountIdForSlot(
                      slot,
                    )}
                  />
                </ConfigFieldCard>
                <ConfigFieldCard
                  label={`discord_bot_${slot}_token`}
                  description={
                    <>
                      Discord bot token for this account.
                      Leave blank to keep current secret
                      unchanged.
                    </>
                  }
                >
                  <TextField.Root
                    className="mt-2"
                    value={String(
                      configDraft[
                        `discord_bot_${slot}_token`
                      ] || "",
                    )}
                    onChange={(e) =>
                      setConfigField(
                        `discord_bot_${slot}_token`,
                        e.target.value,
                      )
                    }
                    placeholder="MTAx..."
                  />
                </ConfigFieldCard>
                <ConfigFieldCard
                  label={`discord_bot_${slot}_allowed_channels`}
                  description={
                    <>
                      Optional allowlist. Only listed
                      channel IDs can trigger this bot.
                    </>
                  }
                >
                  <TextField.Root
                    className="mt-2"
                    value={String(
                      configDraft[
                        `discord_bot_${slot}_allowed_channels_csv`
                      ] || "",
                    )}
                    onChange={(e) =>
                      setConfigField(
                        `discord_bot_${slot}_allowed_channels_csv`,
                        e.target.value,
                      )
                    }
                    placeholder="1234567890,9876543210"
                  />
                </ConfigFieldCard>
                <ConfigFieldCard
                  label={`discord_bot_${slot}_username`}
                  description={
                    <>
                      Optional Discord bot username
                      override.
                    </>
                  }
                >
                  <TextField.Root
                    className="mt-2"
                    value={String(
                      configDraft[
                        `discord_bot_${slot}_username`
                      ] || "",
                    )}
                    onChange={(e) =>
                      setConfigField(
                        `discord_bot_${slot}_username`,
                        e.target.value,
                      )
                    }
                    placeholder={
                      slot === 1
                        ? "discord_main_bot"
                        : `discord_bot_${slot}`
                    }
                  />
                </ConfigFieldCard>
                <ConfigFieldCard
                  label={`discord_bot_${slot}_provider_preset`}
                  description={
                    <>
                      Optional Discord bot LLM provider
                      profile override.
                    </>
                  }
                >
                  <div className="mt-2">
                    <Select.Root
                      value={
                        String(
                          configDraft[
                            `discord_bot_${slot}_provider_preset`
                          ] || "",
                        ) || MAIN_PROFILE_VALUE
                      }
                      onValueChange={(value) =>
                        setConfigField(
                          `discord_bot_${slot}_provider_preset`,
                          value === MAIN_PROFILE_VALUE
                            ? ""
                            : value,
                        )
                      }
                    >
                      <Select.Trigger
                        className="w-full mc-select-trigger-full"
                        placeholder="Select provider profile"
                      />
                      <Select.Content>
                        {providerProfileOptions(
                          providerProfileDrafts,
                          configDraft[
                            `discord_bot_${slot}_provider_preset`
                          ],
                        ).map((option) => (
                          <Select.Item
                            key={`discord-bot-${slot}-provider-preset-${option.value}`}
                            value={option.value}
                          >
                            {option.label}
                          </Select.Item>
                        ))}
                      </Select.Content>
                    </Select.Root>
                  </div>
                </ConfigFieldCard>
              </div>
            </Card>
          );
        })}
      </div>
    </div>
  );
}
