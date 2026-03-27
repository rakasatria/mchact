import React from "react";
import { Card, Select, Text, TextField } from "@radix-ui/themes";
import { ConfigFieldCard } from "../config-field-card";
import { ConfigStepsCard } from "../config-steps-card";
import { SoulPathPickerField } from "../soul-path-picker";
import { useSettings } from "../../context/settings-context";
import { BOT_SLOT_MAX, MAIN_PROFILE_VALUE } from "../../lib/constants";
import {
  normalizeBotCount,
  defaultTelegramAccountIdForSlot,
} from "../../lib/config-helpers";
import { providerProfileOptions } from "../../lib/provider-profiles";

export function TelegramTab(): React.ReactElement {
  const {
    configDraft,
    setConfigField,
    soulFiles,
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
        Telegram
      </Text>
      <ConfigStepsCard
        steps={[
          <>
            Open Telegram and chat with{" "}
            <code>@BotFather</code>.
          </>,
          <>
            Run <code>/newbot</code>, set name and username
            (must end with <code>bot</code>).
          </>,
          <>Copy the bot token and paste below.</>,
          <>
            Configure one or more bot accounts; each account
            can set its own username.
          </>,
          <>
            In groups, mention the bot to trigger replies.
          </>,
        ]}
      />
      <Text size="1" color="gray" className="mt-3 block">
        Configure one or more bots (up to 10). Leave token
        blank to keep existing secret unchanged.
      </Text>
      <div className="mt-4 space-y-3">
        <ConfigFieldCard
          label="telegram_default_account"
          description={
            <>
              Default account id under{" "}
              <code>channels.telegram.accounts</code>.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(
              configDraft.telegram_account_id || "main",
            )}
            onChange={(e) =>
              setConfigField(
                "telegram_account_id",
                e.target.value,
              )
            }
            placeholder="main"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="telegram_bot_count"
          description={
            <>
              Number of Telegram bot accounts to configure
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
              configDraft.telegram_bot_count || 1,
            )}
            onChange={(e) =>
              setConfigField(
                "telegram_bot_count",
                normalizeBotCount(e.target.value),
              )
            }
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="telegram_provider_preset"
          description={
            <>
              Optional Telegram channel-level LLM provider
              profile override.
            </>
          }
        >
          <div className="mt-2">
            <Select.Root
              value={
                String(
                  configDraft.telegram_provider_preset ||
                    "",
                ) || MAIN_PROFILE_VALUE
              }
              onValueChange={(value) =>
                setConfigField(
                  "telegram_provider_preset",
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
                  configDraft.telegram_provider_preset,
                ).map((option) => (
                  <Select.Item
                    key={`telegram-provider-preset-${option.value}`}
                    value={option.value}
                  >
                    {option.label}
                  </Select.Item>
                ))}
              </Select.Content>
            </Select.Root>
          </div>
        </ConfigFieldCard>
        <ConfigFieldCard
          label="telegram_allowed_user_ids"
          description={
            <>
              Optional channel-level allowlist. Accepts CSV
              or JSON array (for example{" "}
              <code>123,456</code> or <code>[123,456]</code>
              ). Merged with each bot account&apos;s{" "}
              <code>allowed_user_ids</code>.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(
              configDraft.telegram_allowed_user_ids || "",
            )}
            onChange={(e) =>
              setConfigField(
                "telegram_allowed_user_ids",
                e.target.value,
              )
            }
            placeholder="123456789,987654321"
          />
        </ConfigFieldCard>
        {Array.from({
          length: normalizeBotCount(
            configDraft.telegram_bot_count || 1,
          ),
        }).map((_, idx) => {
          const slot = idx + 1;
          return (
            <Card
              key={`telegram-bot-${slot}`}
              className="p-3"
            >
              <Text size="2" weight="medium">
                Telegram bot #{slot}
              </Text>
              <div className="mt-2 space-y-3">
                <ConfigFieldCard
                  label={`telegram_bot_${slot}_account_id`}
                  description={
                    <>
                      Bot account id used under{" "}
                      <code>
                        channels.telegram.accounts
                      </code>
                      .
                    </>
                  }
                >
                  <TextField.Root
                    className="mt-2"
                    value={String(
                      configDraft[
                        `telegram_bot_${slot}_account_id`
                      ] ||
                        defaultTelegramAccountIdForSlot(
                          slot,
                        ),
                    )}
                    onChange={(e) =>
                      setConfigField(
                        `telegram_bot_${slot}_account_id`,
                        e.target.value,
                      )
                    }
                    placeholder={defaultTelegramAccountIdForSlot(
                      slot,
                    )}
                  />
                </ConfigFieldCard>
                <ConfigFieldCard
                  label={`telegram_bot_${slot}_token`}
                  description={
                    <>
                      BotFather token for this account.
                      Leave blank to keep current secret
                      unchanged.
                    </>
                  }
                >
                  <TextField.Root
                    className="mt-2"
                    value={String(
                      configDraft[
                        `telegram_bot_${slot}_token`
                      ] || "",
                    )}
                    onChange={(e) =>
                      setConfigField(
                        `telegram_bot_${slot}_token`,
                        e.target.value,
                      )
                    }
                    placeholder="123456789:AA..."
                  />
                </ConfigFieldCard>
                <ConfigFieldCard
                  label={`telegram_bot_${slot}_username`}
                  description={
                    <>
                      Telegram username without{" "}
                      <code>@</code>, used for group mention
                      trigger.
                    </>
                  }
                >
                  <TextField.Root
                    className="mt-2"
                    value={String(
                      configDraft[
                        `telegram_bot_${slot}_username`
                      ] || "",
                    )}
                    onChange={(e) =>
                      setConfigField(
                        `telegram_bot_${slot}_username`,
                        e.target.value,
                      )
                    }
                    placeholder={
                      slot === 1
                        ? "my_main_bot"
                        : `my_bot_${slot}`
                    }
                  />
                </ConfigFieldCard>
                <ConfigFieldCard
                  label={`telegram_bot_${slot}_provider_preset`}
                  description={
                    <>
                      Optional Telegram bot LLM provider
                      profile override.
                    </>
                  }
                >
                  <div className="mt-2">
                    <Select.Root
                      value={
                        String(
                          configDraft[
                            `telegram_bot_${slot}_provider_preset`
                          ] || "",
                        ) || MAIN_PROFILE_VALUE
                      }
                      onValueChange={(value) =>
                        setConfigField(
                          `telegram_bot_${slot}_provider_preset`,
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
                            `telegram_bot_${slot}_provider_preset`
                          ],
                        ).map((option) => (
                          <Select.Item
                            key={`telegram-bot-${slot}-provider-preset-${option.value}`}
                            value={option.value}
                          >
                            {option.label}
                          </Select.Item>
                        ))}
                      </Select.Content>
                    </Select.Root>
                  </div>
                </ConfigFieldCard>
                <ConfigFieldCard
                  label={`telegram_bot_${slot}_soul_path`}
                  description={
                    <>
                      Per-bot soul file. Select from{" "}
                      <code>
                        {String(
                          configDraft.souls_dir || "",
                        ).trim() || "souls"}
                        /*.md
                      </code>{" "}
                      or input a custom filename/path.
                    </>
                  }
                >
                  <SoulPathPickerField
                    value={
                      configDraft[
                        `telegram_bot_${slot}_soul_path`
                      ]
                    }
                    soulsDir={configDraft.souls_dir}
                    soulFiles={soulFiles}
                    onChange={(next) =>
                      setConfigField(
                        `telegram_bot_${slot}_soul_path`,
                        next,
                      )
                    }
                  />
                </ConfigFieldCard>
                <ConfigFieldCard
                  label={`telegram_bot_${slot}_allowed_user_ids`}
                  description={
                    <>
                      Optional per-bot private-chat
                      allowlist (CSV or JSON array).
                    </>
                  }
                >
                  <TextField.Root
                    className="mt-2"
                    value={String(
                      configDraft[
                        `telegram_bot_${slot}_allowed_user_ids`
                      ] || "",
                    )}
                    onChange={(e) =>
                      setConfigField(
                        `telegram_bot_${slot}_allowed_user_ids`,
                        e.target.value,
                      )
                    }
                    placeholder="123456789,987654321"
                  />
                </ConfigFieldCard>
              </div>
            </Card>
          );
        })}
      </div>
    </div>
  );
}
