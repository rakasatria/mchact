import React from "react";
import {
  Badge,
  Button,
  Callout,
  Card,
  Dialog,
  Flex,
  Separator,
  Tabs,
  Text,
} from "@radix-ui/themes";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { SettingsProvider } from "../../context/settings-context";
import type { SettingsContextValue } from "../../context/settings-context";
import type {
  Appearance,
  ConfigPayload,
  ConfigSelfCheck,
} from "../../lib/types";
import { DOCS_BASE } from "../../lib/constants";
import { DYNAMIC_CHANNELS } from "../../lib/channels";
import { TAB_ICONS, CHANNEL_ICONS } from "../../lib/icons";
import { ApiKeysSettings } from "../api-keys-settings";
import { SkillsSettings } from "../skills-settings";
import { McpSettings } from "../mcp-settings";
import { GeneralTab } from "./general-tab";
import { ModelTab } from "./model-tab";
import { TelegramTab } from "./telegram-tab";
import { DiscordTab } from "./discord-tab";
import { IrcTab } from "./irc-tab";
import { DynamicChannelTab } from "./dynamic-channel-tab";
import { WebTab } from "./web-tab";
import { A2ATab } from "./a2a-tab";
import { MultimodalTab } from "./multimodal-tab";

export type SettingsDialogProps = {
  configOpen: boolean;
  setConfigOpen: (open: boolean) => void;
  configLoading: boolean;
  configLoadStage: string;
  configLoadError: string;
  config: ConfigPayload | null;
  configSelfCheck: ConfigSelfCheck | null;
  configSelfCheckLoading: boolean;
  configSelfCheckError: string;
  saveStatus: string;
  onSave: () => void;
  appearance: Appearance;
  authAuthenticated: boolean;
  onLogout: () => void;
  settingsContext: SettingsContextValue;
};

export function SettingsDialog(props: SettingsDialogProps): React.ReactElement {
  const {
    configOpen,
    setConfigOpen,
    configLoading,
    configLoadStage,
    configLoadError,
    config,
    configSelfCheck,
    configSelfCheckLoading,
    configSelfCheckError,
    saveStatus,
    onSave,
    appearance,
    authAuthenticated,
    onLogout,
    settingsContext,
  } = props;

  const { sectionCardClass, sectionCardStyle } = settingsContext;

  return (
    <Dialog.Root open={configOpen} onOpenChange={setConfigOpen}>
      <Dialog.Content
        maxWidth="1120px"
        className="overflow-hidden flex flex-col"
        style={{
          width: "1120px",
          height: "760px",
          maxWidth: "1120px",
          maxHeight: "760px",
        }}
      >
        <Dialog.Title>Settings</Dialog.Title>
        <Dialog.Description size="2" mb="3">
          Channel-first configuration. Save writes to microclaw.config.yaml.
          Restart is required.
        </Dialog.Description>
        {configSelfCheck ? (
          <Callout.Root
            color={
              configSelfCheck.risk_level === "high"
                ? "red"
                : configSelfCheck.risk_level === "medium"
                  ? "orange"
                  : "green"
            }
            size="1"
            variant="soft"
            className="mb-2"
          >
            <Callout.Text>
              Config self-check: risk=
              {String(configSelfCheck.risk_level || "none")}, warnings=
              {Number(configSelfCheck.warning_count || 0)}.{" "}
              <a
                href={`${DOCS_BASE}/configuration`}
                target="_blank"
                rel="noreferrer"
                className="underline"
              >
                Docs
              </a>
            </Callout.Text>
          </Callout.Root>
        ) : null}
        {configSelfCheck?.security_posture ? (
          <details className="mb-2">
            <summary className="cursor-pointer text-sm text-[color:var(--gray-11)]">
              Security posture details
            </summary>
            <Card className="mt-2 p-3">
              <Text size="2" weight="bold">
                Security posture{" "}
                <a
                  href={`${DOCS_BASE}/permissions`}
                  target="_blank"
                  rel="noreferrer"
                  className="underline text-[color:var(--mc-accent)]"
                >
                  Rules
                </a>
              </Text>
              <Text size="1" color="gray" className="mt-1 block">
                sandbox=
                {String(
                  configSelfCheck.security_posture.sandbox_mode || "off",
                )}{" "}
                | runtime=
                {String(
                  Boolean(
                    configSelfCheck.security_posture.sandbox_runtime_available,
                  ),
                )}{" "}
                | backend=
                {String(
                  configSelfCheck.security_posture.sandbox_backend || "auto",
                )}
              </Text>
              <Text size="1" color="gray" className="mt-1 block">
                mount allowlist:{" "}
                {String(
                  configSelfCheck.security_posture.mount_allowlist?.path ||
                    "(default)",
                )}{" "}
                | exists=
                {String(
                  Boolean(
                    configSelfCheck.security_posture.mount_allowlist?.exists,
                  ),
                )}{" "}
                | has_entries=
                {String(
                  Boolean(
                    configSelfCheck.security_posture.mount_allowlist
                      ?.has_entries,
                  ),
                )}
              </Text>
              <div className="mt-2 flex flex-wrap gap-2">
                {(
                  configSelfCheck.security_posture.execution_policies || []
                ).map((p, idx) => (
                  <Badge
                    key={`${String(p.tool)}-${idx}`}
                    color={
                      p.risk === "high"
                        ? "red"
                        : p.risk === "medium"
                          ? "orange"
                          : "gray"
                    }
                    variant="soft"
                  >
                    {String(p.tool)}: {String(p.policy)}
                  </Badge>
                ))}
              </div>
            </Card>
          </details>
        ) : null}
        {configSelfCheckLoading ? (
          <Text size="1" color="gray" className="mb-2 block">
            Checking critical config risks...
          </Text>
        ) : null}
        {configLoadError ? (
          <Callout.Root color="red" size="1" variant="soft" className="mb-2">
            <Callout.Text>{configLoadError}</Callout.Text>
          </Callout.Root>
        ) : null}
        {configSelfCheckError ? (
          <Callout.Root color="red" size="1" variant="soft" className="mb-2">
            <Callout.Text>
              Self-check failed: {configSelfCheckError}
            </Callout.Text>
          </Callout.Root>
        ) : null}
        <div className="mt-2 min-h-0 flex-1">
          {configLoading ? (
            <Card className="mb-3 p-4" style={sectionCardStyle}>
              <Text size="3" weight="bold">
                Loading Runtime Config
              </Text>
              <Text size="1" color="gray" className="mt-2 block">
                {configLoadStage || "Opening settings..."}
              </Text>
              <div
                className="mt-3 h-2 overflow-hidden rounded-full"
                style={{
                  background:
                    "color-mix(in srgb, var(--mc-border-soft) 60%, transparent)",
                }}
              >
                <div
                  className="h-full w-2/5 animate-pulse rounded-full"
                  style={{ background: "var(--mc-accent)" }}
                />
              </div>
            </Card>
          ) : null}
          {config ? (
            <SettingsProvider value={settingsContext}>
              <Tabs.Root
                defaultValue="general"
                orientation="vertical"
                className="h-full min-h-0"
              >
                <div className="grid h-full grid-cols-[240px_minmax(0,1fr)] gap-4">
                  <Card className="h-full p-3" style={sectionCardStyle}>
                    <Tabs.List className="mc-settings-tabs-list flex h-full w-full flex-col gap-1">
                      <Text
                        size="1"
                        color="gray"
                        className="px-2 pt-1 uppercase tracking-wide"
                      >
                        Runtime
                      </Text>
                      <Tabs.Trigger
                        value="general"
                        className="mc-settings-tab-trigger w-full justify-start rounded-lg px-3 py-2 text-[18px] leading-6 bg-transparent data-[state=active]:bg-emerald-500/20 data-[state=active]:text-emerald-200 hover:bg-white/8"
                      >
                        <FontAwesomeIcon
                          icon={TAB_ICONS.general}
                          className="mr-2"
                        />{" "}
                        General
                      </Tabs.Trigger>
                      <Tabs.Trigger
                        value="model"
                        className="mc-settings-tab-trigger w-full justify-start rounded-lg px-3 py-2 text-[18px] leading-6 bg-transparent data-[state=active]:bg-emerald-500/20 data-[state=active]:text-emerald-200 hover:bg-white/8"
                      >
                        <FontAwesomeIcon
                          icon={TAB_ICONS.model}
                          className="mr-2"
                        />{" "}
                        Model
                      </Tabs.Trigger>
                      <Tabs.Trigger
                        value="skills"
                        className="mc-settings-tab-trigger w-full justify-start rounded-lg px-3 py-2 text-[18px] leading-6 bg-transparent data-[state=active]:bg-emerald-500/20 data-[state=active]:text-emerald-200 hover:bg-white/8"
                      >
                        <FontAwesomeIcon
                          icon={TAB_ICONS.skills}
                          className="mr-2"
                        />{" "}
                        Skills
                      </Tabs.Trigger>
                      <Tabs.Trigger
                        value="mcp"
                        className="mc-settings-tab-trigger w-full justify-start rounded-lg px-3 py-2 text-[18px] leading-6 bg-transparent data-[state=active]:bg-emerald-500/20 data-[state=active]:text-emerald-200 hover:bg-white/8"
                      >
                        <FontAwesomeIcon
                          icon={TAB_ICONS.mcp}
                          className="mr-2"
                        />{" "}
                        MCP
                      </Tabs.Trigger>

                      <Text
                        size="1"
                        color="gray"
                        className="px-2 pt-3 uppercase tracking-wide"
                      >
                        Channels
                      </Text>
                      <Tabs.Trigger
                        value="telegram"
                        className="mc-settings-tab-trigger w-full justify-start rounded-lg px-3 py-2 text-[18px] leading-6 bg-transparent data-[state=active]:bg-emerald-500/20 data-[state=active]:text-emerald-200 hover:bg-white/8"
                      >
                        <FontAwesomeIcon
                          icon={TAB_ICONS.telegram}
                          className="mr-2"
                        />{" "}
                        Telegram
                      </Tabs.Trigger>
                      <Tabs.Trigger
                        value="discord"
                        className="mc-settings-tab-trigger w-full justify-start rounded-lg px-3 py-2 text-[18px] leading-6 bg-transparent data-[state=active]:bg-emerald-500/20 data-[state=active]:text-emerald-200 hover:bg-white/8"
                      >
                        <FontAwesomeIcon
                          icon={TAB_ICONS.discord}
                          className="mr-2"
                        />{" "}
                        Discord
                      </Tabs.Trigger>
                      <Tabs.Trigger
                        value="irc"
                        className="mc-settings-tab-trigger w-full justify-start rounded-lg px-3 py-2 text-[18px] leading-6 bg-transparent data-[state=active]:bg-emerald-500/20 data-[state=active]:text-emerald-200 hover:bg-white/8"
                      >
                        <FontAwesomeIcon
                          icon={TAB_ICONS.irc}
                          className="mr-2"
                        />{" "}
                        IRC
                      </Tabs.Trigger>
                      {DYNAMIC_CHANNELS.map((ch) => (
                        <Tabs.Trigger
                          key={ch.name}
                          value={ch.name}
                          className="mc-settings-tab-trigger w-full justify-start rounded-lg px-3 py-2 text-[18px] leading-6 bg-transparent data-[state=active]:bg-emerald-500/20 data-[state=active]:text-emerald-200 hover:bg-white/8"
                        >
                          <FontAwesomeIcon
                            icon={CHANNEL_ICONS[ch.name]}
                            className="mr-2"
                          />{" "}
                          {ch.title}
                        </Tabs.Trigger>
                      ))}

                      <Text
                        size="1"
                        color="gray"
                        className="px-2 pt-3 uppercase tracking-wide"
                      >
                        Integrations
                      </Text>
                      <Tabs.Trigger
                        value="web"
                        className="mc-settings-tab-trigger w-full justify-start rounded-lg px-3 py-2 text-[18px] leading-6 bg-transparent data-[state=active]:bg-emerald-500/20 data-[state=active]:text-emerald-200 hover:bg-white/8"
                      >
                        <FontAwesomeIcon
                          icon={TAB_ICONS.web}
                          className="mr-2"
                        />{" "}
                        Web
                      </Tabs.Trigger>
                      <Tabs.Trigger
                        value="access"
                        className="mc-settings-tab-trigger w-full justify-start rounded-lg px-3 py-2 text-[18px] leading-6 bg-transparent data-[state=active]:bg-emerald-500/20 data-[state=active]:text-emerald-200 hover:bg-white/8"
                      >
                        <FontAwesomeIcon
                          icon={TAB_ICONS.access}
                          className="mr-2"
                        />{" "}
                        Access
                      </Tabs.Trigger>
                      <Tabs.Trigger
                        value="a2a"
                        className="mc-settings-tab-trigger w-full justify-start rounded-lg px-3 py-2 text-[18px] leading-6 bg-transparent data-[state=active]:bg-emerald-500/20 data-[state=active]:text-emerald-200 hover:bg-white/8"
                      >
                        <FontAwesomeIcon
                          icon={TAB_ICONS.a2a}
                          className="mr-2"
                        />{" "}
                        A2A
                      </Tabs.Trigger>
                      <Tabs.Trigger
                        value="multimodal"
                        className="mc-settings-tab-trigger w-full justify-start rounded-lg px-3 py-2 text-[18px] leading-6 bg-transparent data-[state=active]:bg-emerald-500/20 data-[state=active]:text-emerald-200 hover:bg-white/8"
                      >
                        <FontAwesomeIcon
                          icon={TAB_ICONS.multimodal}
                          className="mr-2"
                        />{" "}
                        Multimodal
                      </Tabs.Trigger>
                      {authAuthenticated ? (
                        <div className="mt-auto pt-3">
                          <Separator size="4" />
                          <button
                            type="button"
                            onClick={onLogout}
                            className="mt-3 inline-flex w-full items-center justify-center rounded-lg border px-3 py-2 text-sm font-medium transition hover:brightness-110 active:brightness-95"
                            style={
                              appearance === "dark"
                                ? {
                                    borderColor: "var(--mc-border-soft)",
                                    background: "var(--mc-bg-panel)",
                                    color: "var(--mc-text)",
                                  }
                                : undefined
                            }
                          >
                            Log out
                          </button>
                        </div>
                      ) : null}
                    </Tabs.List>
                  </Card>

                  <div className="min-w-0 overflow-y-auto pr-1">
                    <Tabs.Content value="general">
                      <GeneralTab />
                    </Tabs.Content>

                    <Tabs.Content value="model">
                      <ModelTab />
                    </Tabs.Content>

                    <Tabs.Content value="skills">
                      <div
                        className={sectionCardClass}
                        style={sectionCardStyle}
                      >
                        <Text size="3" weight="bold">
                          Skills
                        </Text>
                        <SkillsSettings />
                      </div>
                    </Tabs.Content>

                    <Tabs.Content value="mcp">
                      <div
                        className={sectionCardClass}
                        style={sectionCardStyle}
                      >
                        <Text size="3" weight="bold">
                          MCP Servers
                        </Text>
                        <Text size="1" color="gray" className="mt-1 block">
                          View connected Model Context Protocol servers and
                          their available tools.
                        </Text>
                        <div className="mt-4">
                          <McpSettings />
                        </div>
                      </div>
                    </Tabs.Content>

                    <Tabs.Content value="access">
                      <div
                        className={sectionCardClass}
                        style={sectionCardStyle}
                      >
                        <Text size="3" weight="bold">
                          Access
                        </Text>
                        <Text size="1" color="gray" className="mt-1 block">
                          Manage operator API keys for Mission Control, scripts,
                          and automation.
                        </Text>
                        <div className="mt-4">
                          <ApiKeysSettings
                            open={configOpen}
                            authenticated={authAuthenticated}
                          />
                        </div>
                      </div>
                    </Tabs.Content>

                    <Tabs.Content value="telegram">
                      <TelegramTab />
                    </Tabs.Content>

                    <Tabs.Content value="discord">
                      <DiscordTab />
                    </Tabs.Content>

                    <Tabs.Content value="irc">
                      <IrcTab />
                    </Tabs.Content>

                    {DYNAMIC_CHANNELS.map((ch) => (
                      <Tabs.Content key={ch.name} value={ch.name}>
                        <DynamicChannelTab channelDef={ch} />
                      </Tabs.Content>
                    ))}

                    <Tabs.Content value="web">
                      <WebTab />
                    </Tabs.Content>

                    <Tabs.Content value="a2a">
                      <A2ATab />
                    </Tabs.Content>

                    <Tabs.Content value="multimodal">
                      <MultimodalTab />
                    </Tabs.Content>
                  </div>
                </div>
              </Tabs.Root>
            </SettingsProvider>
          ) : !configLoading && !configLoadError ? (
            <Text size="2" color="gray">
              Runtime config is unavailable.
            </Text>
          ) : null}
        </div>

        <div className="mt-3 flex items-center justify-between border-t border-[color:var(--mc-border-soft)] pt-3">
          {saveStatus ? (
            <Text
              size="2"
              color={saveStatus.startsWith("Save failed") ? "red" : "green"}
            >
              {saveStatus}
            </Text>
          ) : (
            <span />
          )}
          <Flex justify="end" gap="2">
            <Dialog.Close>
              <Button variant="soft">Close</Button>
            </Dialog.Close>
            <Button onClick={onSave} disabled={configLoading || !config}>
              Save
            </Button>
          </Flex>
        </div>
      </Dialog.Content>
    </Dialog.Root>
  );
}
