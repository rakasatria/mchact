import React from "react";
import { Select, Text, TextField } from "@radix-ui/themes";
import { ConfigFieldCard } from "../config-field-card";
import { ConfigStepsCard } from "../config-steps-card";
import { useSettings } from "../../context/settings-context";
import { MAIN_PROFILE_VALUE } from "../../lib/constants";
import { providerProfileOptions } from "../../lib/provider-profiles";

export function IrcTab(): React.ReactElement {
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
        IRC
      </Text>
      <ConfigStepsCard
        steps={[
          <>Set IRC server and nick.</>,
          <>
            Set channels as comma-separated list, for
            example <code>#general,#bot</code>.
          </>,
          <>
            Use TLS fields when connecting to secure
            endpoints.
          </>,
        ]}
      />
      <Text size="1" color="gray" className="mt-3 block">
        Required for IRC runtime: server and nick.
      </Text>
      <div className="mt-4 space-y-3">
        <ConfigFieldCard
          label="irc_server"
          description={<>IRC server hostname.</>}
        >
          <TextField.Root
            className="mt-2"
            value={String(configDraft.irc_server || "")}
            onChange={(e) =>
              setConfigField("irc_server", e.target.value)
            }
            placeholder="irc.libera.chat"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="irc_port"
          description={
            <>
              IRC server port. Typical values: 6667 or 6697
              (TLS).
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(configDraft.irc_port || "")}
            onChange={(e) =>
              setConfigField("irc_port", e.target.value)
            }
            placeholder="6667"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="irc_nick"
          description={<>Bot nickname.</>}
        >
          <TextField.Root
            className="mt-2"
            value={String(configDraft.irc_nick || "")}
            onChange={(e) =>
              setConfigField("irc_nick", e.target.value)
            }
            placeholder="mchact"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="irc_username"
          description={
            <>
              Optional IRC username. Defaults to nick when
              empty.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(configDraft.irc_username || "")}
            onChange={(e) =>
              setConfigField("irc_username", e.target.value)
            }
            placeholder="mchact"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="irc_real_name"
          description={
            <>Optional IRC real name/gecos field.</>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(configDraft.irc_real_name || "")}
            onChange={(e) =>
              setConfigField(
                "irc_real_name",
                e.target.value,
              )
            }
            placeholder="mchact"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="irc_channels"
          description={
            <>Comma-separated target channels.</>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(configDraft.irc_channels || "")}
            onChange={(e) =>
              setConfigField("irc_channels", e.target.value)
            }
            placeholder="#general,#support"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="irc_password"
          description={
            <>
              Optional IRC server password. Leave blank to
              keep current secret unchanged.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(configDraft.irc_password || "")}
            onChange={(e) =>
              setConfigField("irc_password", e.target.value)
            }
            placeholder="password"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="irc_mention_required"
          description={
            <>
              In channels, require bot mention before
              responding (<code>true</code>/
              <code>false</code>).
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(
              configDraft.irc_mention_required || "",
            )}
            onChange={(e) =>
              setConfigField(
                "irc_mention_required",
                e.target.value,
              )
            }
            placeholder="true"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="irc_tls"
          description={
            <>
              Enable TLS (<code>true</code>/
              <code>false</code>).
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(configDraft.irc_tls || "")}
            onChange={(e) =>
              setConfigField("irc_tls", e.target.value)
            }
            placeholder="false"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="irc_tls_server_name"
          description={
            <>
              Optional TLS SNI server name. Defaults to
              server.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(
              configDraft.irc_tls_server_name || "",
            )}
            onChange={(e) =>
              setConfigField(
                "irc_tls_server_name",
                e.target.value,
              )
            }
            placeholder="irc.libera.chat"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="irc_tls_danger_accept_invalid_certs"
          description={
            <>
              Allow invalid TLS certs (<code>true</code>/
              <code>false</code>). Only for testing.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(
              configDraft.irc_tls_danger_accept_invalid_certs ||
                "",
            )}
            onChange={(e) =>
              setConfigField(
                "irc_tls_danger_accept_invalid_certs",
                e.target.value,
              )
            }
            placeholder="false"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="irc_provider_preset"
          description={
            <>Optional IRC LLM provider profile override.</>
          }
        >
          <div className="mt-2">
            <Select.Root
              value={
                String(
                  configDraft.irc_provider_preset || "",
                ) || MAIN_PROFILE_VALUE
              }
              onValueChange={(value) =>
                setConfigField(
                  "irc_provider_preset",
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
                  configDraft.irc_provider_preset,
                ).map((option) => (
                  <Select.Item
                    key={`irc-provider-preset-${option.value}`}
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
    </div>
  );
}
