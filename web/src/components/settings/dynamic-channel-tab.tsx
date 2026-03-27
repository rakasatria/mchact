import React from "react";
import { Card, Select, Text, TextField } from "@radix-ui/themes";
import { ConfigFieldCard } from "../config-field-card";
import { ConfigStepsCard } from "../config-steps-card";
import { SoulPathPickerField } from "../soul-path-picker";
import { useSettings } from "../../context/settings-context";
import { BOT_SLOT_MAX, MAIN_PROFILE_VALUE } from "../../lib/constants";
import {
  normalizeBotCount,
  defaultAccountIdForSlot,
} from "../../lib/config-helpers";
import { providerProfileOptions } from "../../lib/provider-profiles";
import type { DynChannelDef } from "../../lib/types";

type DynamicChannelTabProps = {
  channelDef: DynChannelDef;
};

export function DynamicChannelTab({
  channelDef: ch,
}: DynamicChannelTabProps): React.ReactElement {
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
        {ch.title}
      </Text>
      <ConfigStepsCard
        steps={ch.steps.map((s, i) => (
          <span key={i}>{s}</span>
        ))}
      />
      <Text size="1" color="gray" className="mt-3 block">
        {ch.hint}
      </Text>
      <div className="mt-4 space-y-3">
        {(ch.channelFields || []).map((f) => {
          const stateKey = `${ch.name}__${f.yamlKey}`;
          const hasExistingSecret = f.secret
            ? Boolean(
                configDraft[
                  `${ch.name}__has__${f.yamlKey}`
                ],
              )
            : false;
          return (
            <ConfigFieldCard
              key={stateKey}
              label={`${ch.name}_${f.yamlKey}`}
              description={<>{f.description}</>}
            >
              <TextField.Root
                className="mt-2"
                type={
                  f.valueType === "number"
                    ? "number"
                    : "text"
                }
                min={
                  f.valueType === "number"
                    ? "0"
                    : undefined
                }
                step={
                  f.valueType === "number"
                    ? "1"
                    : undefined
                }
                value={String(
                  configDraft[stateKey] || "",
                )}
                onChange={(e) =>
                  setConfigField(stateKey, e.target.value)
                }
                placeholder={f.placeholder}
              />
              {hasExistingSecret &&
              !String(
                configDraft[stateKey] || "",
              ).trim() ? (
                <Text
                  size="1"
                  color="gray"
                  className="mt-2 block"
                >
                  Existing secret is configured and will
                  be preserved.
                </Text>
              ) : null}
            </ConfigFieldCard>
          );
        })}
        <ConfigFieldCard
          key={`${ch.name}__account_id`}
          label={`${ch.name}_default_account`}
          description={
            <>
              Default account id under{" "}
              <code>channels.{ch.name}.accounts</code>.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(
              configDraft[`${ch.name}__account_id`] ||
                "main",
            )}
            onChange={(e) =>
              setConfigField(
                `${ch.name}__account_id`,
                e.target.value,
              )
            }
            placeholder="main"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          key={`${ch.name}__bot_count`}
          label={`${ch.name}_bot_count`}
          description={
            <>
              Number of bot accounts to configure for{" "}
              <code>{ch.name}</code> (1-10).
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            type="number"
            min="1"
            max={String(BOT_SLOT_MAX)}
            value={String(
              configDraft[`${ch.name}__bot_count`] || 1,
            )}
            onChange={(e) =>
              setConfigField(
                `${ch.name}__bot_count`,
                normalizeBotCount(e.target.value),
              )
            }
          />
        </ConfigFieldCard>
        {Array.from({
          length: normalizeBotCount(
            configDraft[`${ch.name}__bot_count`] || 1,
          ),
        }).map((_, idx) => {
          const slot = idx + 1;
          return (
            <Card
              key={`${ch.name}-bot-${slot}`}
              className="p-3"
            >
              <Text size="2" weight="medium">
                {ch.title} bot #{slot}
              </Text>
              <div className="mt-2 space-y-3">
                <ConfigFieldCard
                  key={`${ch.name}__bot_${slot}__account_id`}
                  label={`${ch.name}_bot_${slot}_account_id`}
                  description={
                    <>
                      Bot account id used under{" "}
                      <code>
                        channels.{ch.name}.accounts
                      </code>
                      .
                    </>
                  }
                >
                  <TextField.Root
                    className="mt-2"
                    value={String(
                      configDraft[
                        `${ch.name}__bot_${slot}__account_id`
                      ] || defaultAccountIdForSlot(slot),
                    )}
                    onChange={(e) =>
                      setConfigField(
                        `${ch.name}__bot_${slot}__account_id`,
                        e.target.value,
                      )
                    }
                    placeholder={defaultAccountIdForSlot(
                      slot,
                    )}
                  />
                </ConfigFieldCard>
                <ConfigFieldCard
                  key={`${ch.name}__bot_${slot}__soul_path`}
                  label={`${ch.name}_bot_${slot}_soul_path`}
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
                        `${ch.name}__bot_${slot}__soul_path`
                      ]
                    }
                    soulsDir={configDraft.souls_dir}
                    soulFiles={soulFiles}
                    onChange={(next) =>
                      setConfigField(
                        `${ch.name}__bot_${slot}__soul_path`,
                        next,
                      )
                    }
                  />
                </ConfigFieldCard>
                {ch.fields.map((f) => {
                  const stateKey = `${ch.name}__bot_${slot}__${f.yamlKey}`;
                  const hasExistingSecret = f.secret
                    ? Boolean(
                        configDraft[
                          `${ch.name}__bot_${slot}__has__${f.yamlKey}`
                        ],
                      )
                    : false;
                  if (f.yamlKey === "provider_preset") {
                    return (
                      <ConfigFieldCard
                        key={stateKey}
                        label={`${ch.name}_bot_${slot}_${f.yamlKey}`}
                        description={<>{f.description}</>}
                      >
                        <div className="mt-2">
                          <Select.Root
                            value={
                              String(
                                configDraft[stateKey] ||
                                  "",
                              ) || MAIN_PROFILE_VALUE
                            }
                            onValueChange={(value) =>
                              setConfigField(
                                stateKey,
                                value ===
                                  MAIN_PROFILE_VALUE
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
                                configDraft[stateKey],
                              ).map((option) => (
                                <Select.Item
                                  key={`${stateKey}-${option.value}`}
                                  value={option.value}
                                >
                                  {option.label}
                                </Select.Item>
                              ))}
                            </Select.Content>
                          </Select.Root>
                        </div>
                      </ConfigFieldCard>
                    );
                  }
                  return (
                    <ConfigFieldCard
                      key={stateKey}
                      label={`${ch.name}_bot_${slot}_${f.yamlKey}`}
                      description={<>{f.description}</>}
                    >
                      <TextField.Root
                        className="mt-2"
                        type={
                          f.valueType === "number"
                            ? "number"
                            : "text"
                        }
                        min={
                          f.valueType === "number"
                            ? "0"
                            : undefined
                        }
                        step={
                          f.valueType === "number"
                            ? "1"
                            : undefined
                        }
                        value={String(
                          configDraft[stateKey] || "",
                        )}
                        onChange={(e) =>
                          setConfigField(
                            stateKey,
                            e.target.value,
                          )
                        }
                        placeholder={f.placeholder}
                      />
                      {hasExistingSecret &&
                      !String(
                        configDraft[stateKey] || "",
                      ).trim() ? (
                        <Text
                          size="1"
                          color="gray"
                          className="mt-2 block"
                        >
                          Existing secret is configured
                          and will be preserved.
                        </Text>
                      ) : null}
                    </ConfigFieldCard>
                  );
                })}
              </div>
            </Card>
          );
        })}
      </div>
    </div>
  );
}
