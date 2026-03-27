import React, { useMemo } from "react";
import {
  Badge,
  Button,
  Card,
  Flex,
  Select,
  Text,
  TextField,
} from "@radix-ui/themes";
import { ConfigFieldCard } from "../config-field-card";
import { ConfigToggleCard } from "../config-toggle-card";
import { useSettings } from "../../context/settings-context";
import {
  DEFAULT_CONFIG_VALUES,
  PROVIDER_SUGGESTIONS,
  MODEL_OPTIONS,
} from "../../lib/constants";
import { defaultModelForProvider } from "../../lib/config-helpers";
import {
  nextProviderProfileId,
  providerProfileReferences,
  providerProfileOptions,
} from "../../lib/provider-profiles";

export function ModelTab(): React.ReactElement {
  const {
    configDraft,
    setConfigField,
    providerProfileDrafts,
    addProviderProfile,
    cloneProviderProfile,
    removeProviderProfile,
    resetRefsAndRemoveProviderProfile,
    updateProviderProfile,
    sectionCardClass,
    sectionCardStyle,
    toggleCardClass,
    toggleCardStyle,
  } = useSettings();

  const currentProvider = String(
    configDraft.llm_provider || DEFAULT_CONFIG_VALUES.llm_provider,
  )
    .trim()
    .toLowerCase();
  const providerOptions = Array.from(
    new Set(
      [
        currentProvider,
        ...PROVIDER_SUGGESTIONS.map((p) => p.toLowerCase()),
      ].filter(Boolean),
    ),
  );
  const modelOptions = MODEL_OPTIONS[currentProvider] || [];
  const nextProviderProfileHint = useMemo(
    () => nextProviderProfileId(providerProfileDrafts),
    [providerProfileDrafts],
  );

  return (
    <>
      <div
        className={sectionCardClass}
        style={sectionCardStyle}
      >
        <Text size="3" weight="bold">
          Model
        </Text>
        <Text size="1" color="gray" className="mt-1 block">
          LLM provider and API settings.
        </Text>
        <Text size="1" color="gray" className="mt-2 block">
          Global LLM config acts like the main profile.
          Channel and bot overrides should select a provider
          profile instead of overriding model directly.
        </Text>
        <Text size="1" color="gray" className="mt-1 block">
          For custom providers set <code>llm_base_url</code>.
          For <code>openai-codex</code>, configure
          auth/provider in <code>~/.codex/auth.json</code> and{" "}
          <code>~/.codex/config.toml</code> (this form ignores{" "}
          <code>api_key</code>/<code>llm_base_url</code>).{" "}
          <code>ollama</code> can leave <code>api_key</code>{" "}
          empty.
        </Text>
        <div className="mt-4 space-y-3">
          <ConfigFieldCard
            label="llm_provider"
            description={
              <>
                Select the global main provider backend.
                {currentProvider === "openrouter" ? (
                  <>
                    {" "}
                    Browse models:{" "}
                    <a
                      href="https://openrouter.ai/models"
                      target="_blank"
                      rel="noreferrer"
                    >
                      openrouter.ai/models
                    </a>
                    .
                  </>
                ) : null}
                {currentProvider === "nvidia" ? (
                  <>
                    {" "}
                    Browse models:{" "}
                    <a
                      href="https://build.nvidia.com/models"
                      target="_blank"
                      rel="noreferrer"
                    >
                      build.nvidia.com/models
                    </a>
                    .
                  </>
                ) : null}
              </>
            }
          >
            <div className="mt-2">
              <Select.Root
                value={String(
                  configDraft.llm_provider ||
                    DEFAULT_CONFIG_VALUES.llm_provider,
                )}
                onValueChange={(value) =>
                  setConfigField("llm_provider", value)
                }
              >
                <Select.Trigger
                  className="w-full mc-select-trigger-full"
                  placeholder="Select provider"
                />
                <Select.Content>
                  {providerOptions.map((provider) => (
                    <Select.Item
                      key={provider}
                      value={provider}
                    >
                      {provider}
                    </Select.Item>
                  ))}
                </Select.Content>
              </Select.Root>
            </div>
          </ConfigFieldCard>

          <ConfigFieldCard
            label="model"
            description={
              <>Exact model id to use for requests.</>
            }
          >
            <TextField.Root
              className="mt-2"
              value={String(
                configDraft.model ||
                  defaultModelForProvider(
                    String(
                      configDraft.llm_provider ||
                        DEFAULT_CONFIG_VALUES.llm_provider,
                    ),
                  ),
              )}
              onChange={(e) =>
                setConfigField("model", e.target.value)
              }
              placeholder="claude-sonnet-4-5-20250929"
            />
            {modelOptions.length > 0 ? (
              <Text
                size="1"
                color="gray"
                className="mt-2 block"
              >
                Suggested: {modelOptions.join(" / ")}
              </Text>
            ) : null}
          </ConfigFieldCard>

          <ConfigFieldCard
            label="llm_user_agent"
            description={
              <>
                Optional global HTTP user-agent for LLM
                requests.
              </>
            }
          >
            <TextField.Root
              className="mt-2"
              value={String(configDraft.llm_user_agent || "")}
              onChange={(e) =>
                setConfigField(
                  "llm_user_agent",
                  e.target.value,
                )
              }
              placeholder="microclaw/1.0"
            />
          </ConfigFieldCard>

          {currentProvider === "custom" ? (
            <ConfigFieldCard
              label="llm_base_url"
              description={
                <>
                  Base URL for OpenAI-compatible custom
                  provider endpoint.
                </>
              }
            >
              <TextField.Root
                className="mt-2"
                value={String(configDraft.llm_base_url || "")}
                onChange={(e) =>
                  setConfigField(
                    "llm_base_url",
                    e.target.value,
                  )
                }
                placeholder="https://api.example.com/v1"
              />
            </ConfigFieldCard>
          ) : null}

          <ConfigFieldCard
            label="api_key"
            description={
              currentProvider === "openai-codex" ? (
                <>
                  For <code>openai-codex</code>, this field is
                  ignored. Configure{" "}
                  <code>~/.codex/auth.json</code> instead.
                </>
              ) : (
                <>
                  Provider API key. Leave blank to keep
                  current secret unchanged.
                </>
              )
            }
          >
            <TextField.Root
              className="mt-2"
              value={String(configDraft.api_key || "")}
              onChange={(e) =>
                setConfigField("api_key", e.target.value)
              }
              placeholder={
                currentProvider === "openai-codex"
                  ? "(ignored for openai-codex)"
                  : "sk-..."
              }
            />
          </ConfigFieldCard>
        </div>

        <div className="mt-6">
          <Flex align="center" justify="between" gap="3">
            <div>
              <Text size="3" weight="bold">
                LLM provider profiles
              </Text>
              <Text
                size="1"
                color="gray"
                className="mt-1 block"
              >
                Reusable provider profiles for channel and bot
                overrides.
              </Text>
            </div>
            <Button
              variant="soft"
              onClick={addProviderProfile}
            >
              Add profile
            </Button>
          </Flex>
          <Text size="1" color="gray" className="mt-2 block">
            Next default profile id: {nextProviderProfileHint}
          </Text>
          <div className="mt-3 space-y-3">
            {providerProfileDrafts.length === 0 ? (
              <Card className="p-3">
                <Text size="2">
                  No provider profiles yet. Add one, then
                  point channel or bot overrides at it.
                </Text>
              </Card>
            ) : (
              providerProfileDrafts.map((entry, index) => (
                <Card
                  key={`provider-profile-${index}`}
                  className="p-3"
                >
                  {(() => {
                    const refs = providerProfileReferences(
                      configDraft,
                      entry.id,
                    );
                    const inUse = refs.length > 0;
                    return (
                      <>
                        <Flex
                          align="center"
                          justify="between"
                          gap="3"
                        >
                          <div>
                            <Text size="2" weight="medium">
                              {entry.id ||
                                `Profile #${index + 1}`}
                            </Text>
                            <Text
                              size="1"
                              color="gray"
                              className="mt-1 block"
                            >
                              {entry.provider || "custom"} /{" "}
                              {entry.default_model ||
                                "(no model)"}
                            </Text>
                            <Text
                              size="1"
                              color={inUse ? "amber" : "gray"}
                              className="mt-1 block"
                            >
                              {inUse
                                ? `${refs.length} ref(s) · ${refs.join(", ")}`
                                : "unused"}
                            </Text>
                          </div>
                          <Flex gap="2">
                            <Button
                              variant="soft"
                              onClick={() =>
                                cloneProviderProfile(index)
                              }
                            >
                              Clone
                            </Button>
                            {inUse ? (
                              <Button
                                variant="soft"
                                color="amber"
                                onClick={() =>
                                  resetRefsAndRemoveProviderProfile(
                                    index,
                                  )
                                }
                              >
                                Reset refs + delete
                              </Button>
                            ) : null}
                            <Button
                              variant="soft"
                              color="red"
                              disabled={inUse}
                              onClick={() =>
                                removeProviderProfile(index)
                              }
                            >
                              Delete
                            </Button>
                          </Flex>
                        </Flex>
                        <div className="mt-3 grid grid-cols-1 gap-3 md:grid-cols-2">
                          <ConfigFieldCard
                            label="profile_id"
                            description={
                              <>
                                Unique id for channel/bot
                                profile selection. Avoid{" "}
                                <code>main</code>.
                              </>
                            }
                          >
                            <TextField.Root
                              className="mt-2"
                              value={entry.id}
                              onChange={(e) =>
                                updateProviderProfile(index, {
                                  id: e.target.value,
                                })
                              }
                              placeholder={
                                nextProviderProfileHint
                              }
                            />
                          </ConfigFieldCard>
                          <ConfigFieldCard
                            label="provider"
                            description={
                              <>
                                Provider backend used by this
                                profile.
                              </>
                            }
                          >
                            <div className="mt-2">
                              <Select.Root
                                value={
                                  entry.provider ||
                                  "anthropic"
                                }
                                onValueChange={(value) =>
                                  updateProviderProfile(
                                    index,
                                    {
                                      provider: value,
                                      default_model:
                                        entry.default_model ||
                                        defaultModelForProvider(
                                          value,
                                        ),
                                    },
                                  )
                                }
                              >
                                <Select.Trigger
                                  className="w-full mc-select-trigger-full"
                                  placeholder="Select provider"
                                />
                                <Select.Content>
                                  {providerOptions.map(
                                    (provider) => (
                                      <Select.Item
                                        key={`provider-profile-opt-${index}-${provider}`}
                                        value={provider}
                                      >
                                        {provider}
                                      </Select.Item>
                                    ),
                                  )}
                                </Select.Content>
                              </Select.Root>
                            </div>
                          </ConfigFieldCard>
                          <ConfigFieldCard
                            label="api_key"
                            description={
                              <>
                                Optional API key. Leave blank
                                to keep current secret
                                unchanged.
                              </>
                            }
                          >
                            <TextField.Root
                              className="mt-2"
                              value={entry.api_key}
                              onChange={(e) =>
                                updateProviderProfile(index, {
                                  api_key: e.target.value,
                                })
                              }
                              placeholder="sk-..."
                            />
                          </ConfigFieldCard>
                          <ConfigFieldCard
                            label="llm_base_url"
                            description={
                              <>
                                Optional base URL override for
                                this profile.
                              </>
                            }
                          >
                            <TextField.Root
                              className="mt-2"
                              value={entry.llm_base_url}
                              onChange={(e) =>
                                updateProviderProfile(index, {
                                  llm_base_url:
                                    e.target.value,
                                })
                              }
                              placeholder="https://api.example.com/v1"
                            />
                          </ConfigFieldCard>
                          <ConfigFieldCard
                            label="llm_user_agent"
                            description={
                              <>
                                Optional HTTP user-agent
                                override for this profile.
                              </>
                            }
                          >
                            <TextField.Root
                              className="mt-2"
                              value={entry.llm_user_agent}
                              onChange={(e) =>
                                updateProviderProfile(index, {
                                  llm_user_agent:
                                    e.target.value,
                                })
                              }
                              placeholder="microclaw/1.0"
                            />
                          </ConfigFieldCard>
                          <ConfigFieldCard
                            label="default_model"
                            description={
                              <>
                                Exact model id for this
                                profile.
                              </>
                            }
                          >
                            <TextField.Root
                              className="mt-2"
                              value={entry.default_model}
                              onChange={(e) =>
                                updateProviderProfile(index, {
                                  default_model:
                                    e.target.value,
                                })
                              }
                              placeholder={defaultModelForProvider(
                                entry.provider || "anthropic",
                              )}
                            />
                          </ConfigFieldCard>
                        </div>
                        <div className="mt-3">
                          <ConfigToggleCard
                            label="show_thinking"
                            description={
                              <>
                                Show reasoning text when this
                                profile is selected.
                              </>
                            }
                            checked={entry.show_thinking}
                            onCheckedChange={(checked) =>
                              updateProviderProfile(index, {
                                show_thinking: checked,
                              })
                            }
                            className={toggleCardClass}
                            style={toggleCardStyle}
                          />
                        </div>
                      </>
                    );
                  })()}
                </Card>
              ))
            )}
          </div>
        </div>
      </div>
      <div
        className={`${sectionCardClass} mt-4`}
        style={sectionCardStyle}
      >
        <Text size="3" weight="bold">
          Embedding
        </Text>
        <Text size="1" color="gray" className="mt-1 block">
          Optional embedding runtime settings for semantic
          memory (requires sqlite-vec build).
        </Text>
        <div className="mt-4 space-y-3">
          <ConfigFieldCard
            label="embedding_provider"
            description={
              <>
                Optional runtime embedding provider:{" "}
                <code>openai</code> or <code>ollama</code>.
              </>
            }
          >
            <TextField.Root
              className="mt-2"
              value={String(
                configDraft.embedding_provider || "",
              )}
              onChange={(e) =>
                setConfigField(
                  "embedding_provider",
                  e.target.value,
                )
              }
              placeholder="openai"
            />
          </ConfigFieldCard>
          <ConfigFieldCard
            label="embedding_api_key"
            description={
              <>
                Optional embedding API key. Leave blank to
                keep unchanged.
              </>
            }
          >
            <TextField.Root
              className="mt-2"
              value={String(
                configDraft.embedding_api_key || "",
              )}
              onChange={(e) =>
                setConfigField(
                  "embedding_api_key",
                  e.target.value,
                )
              }
              placeholder="sk-..."
            />
          </ConfigFieldCard>
          <ConfigFieldCard
            label="embedding_base_url"
            description={
              <>Optional embedding base URL override.</>
            }
          >
            <TextField.Root
              className="mt-2"
              value={String(
                configDraft.embedding_base_url || "",
              )}
              onChange={(e) =>
                setConfigField(
                  "embedding_base_url",
                  e.target.value,
                )
              }
              placeholder="https://api.openai.com/v1"
            />
          </ConfigFieldCard>
          <ConfigFieldCard
            label="embedding_model"
            description={<>Optional embedding model id.</>}
          >
            <TextField.Root
              className="mt-2"
              value={String(
                configDraft.embedding_model || "",
              )}
              onChange={(e) =>
                setConfigField(
                  "embedding_model",
                  e.target.value,
                )
              }
              placeholder="text-embedding-3-small"
            />
          </ConfigFieldCard>
          <ConfigFieldCard
            label="embedding_dim"
            description={
              <>Optional embedding vector dimension.</>
            }
          >
            <TextField.Root
              className="mt-2"
              type="number"
              value={String(configDraft.embedding_dim || "")}
              onChange={(e) =>
                setConfigField(
                  "embedding_dim",
                  e.target.value,
                )
              }
              placeholder="1536"
            />
          </ConfigFieldCard>
        </div>
      </div>
    </>
  );
}
