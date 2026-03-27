import React from "react";
import { Button, Card, Text, TextField } from "@radix-ui/themes";
import { ConfigFieldCard } from "../config-field-card";
import { ConfigToggleCard } from "../config-toggle-card";
import { ConfigStepsCard } from "../config-steps-card";
import { useSettings } from "../../context/settings-context";
import type { A2APeerDraft } from "../../lib/types";

export function A2ATab(): React.ReactElement {
  const {
    configDraft,
    setConfigField,
    updateA2APeer,
    addA2APeer,
    removeA2APeer,
    sectionCardClass,
    sectionCardStyle,
    toggleCardClass,
    toggleCardStyle,
  } = useSettings();

  return (
    <div
      className={sectionCardClass}
      style={sectionCardStyle}
    >
      <Text size="3" weight="bold">
        A2A
      </Text>
      <ConfigStepsCard
        steps={[
          <>
            Enable A2A only on instances that should accept
            or send agent-to-agent HTTP traffic.
          </>,
          <>
            Set <code>public_base_url</code> to the
            externally reachable HTTPS origin for this
            instance.
          </>,
          <>
            Configure shared bearer tokens for inbound auth
            and peers JSON for outbound targets.
          </>,
        ]}
      />
      <Text size="1" color="gray" className="mt-3 block">
        <code>a2a.shared_tokens</code> is write-only here
        for safety. Leave it blank to keep existing tokens
        unchanged.
      </Text>
      <div className="mt-4 grid grid-cols-1 gap-3">
        <ConfigToggleCard
          label="a2a_enabled"
          description={
            <>
              Enable A2A HTTP endpoints and built-in
              delegation tools.
            </>
          }
          checked={Boolean(configDraft.a2a_enabled)}
          onCheckedChange={(checked) =>
            setConfigField("a2a_enabled", checked)
          }
          className={toggleCardClass}
          style={toggleCardStyle}
        />
      </div>
      <div className="mt-4 space-y-3">
        <ConfigFieldCard
          label="a2a_public_base_url"
          description={
            <>
              Public HTTPS base URL advertised in the agent
              card.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(
              configDraft.a2a_public_base_url || "",
            )}
            onChange={(e) =>
              setConfigField(
                "a2a_public_base_url",
                e.target.value,
              )
            }
            placeholder="https://planner.example.com"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="a2a_agent_name"
          description={
            <>Friendly agent name shown to remote peers.</>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(configDraft.a2a_agent_name || "")}
            onChange={(e) =>
              setConfigField(
                "a2a_agent_name",
                e.target.value,
              )
            }
            placeholder="Planner"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="a2a_agent_description"
          description={
            <>
              Optional short description for the A2A agent
              card.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(
              configDraft.a2a_agent_description || "",
            )}
            onChange={(e) =>
              setConfigField(
                "a2a_agent_description",
                e.target.value,
              )
            }
            placeholder="Routes work to specialized agents"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="a2a_shared_tokens"
          description={
            <>
              Inbound bearer tokens accepted by{" "}
              <code>/api/a2a/message</code>. CSV or JSON
              array. Leave blank to keep unchanged.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(
              configDraft.a2a_shared_tokens || "",
            )}
            onChange={(e) =>
              setConfigField(
                "a2a_shared_tokens",
                e.target.value,
              )
            }
            placeholder='["shared-a2a-token"]'
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="a2a_peers"
          description={
            <>
              Outbound peers used by <code>a2a_send</code>.
              Add one card per remote agent.
            </>
          }
        >
          <div className="space-y-3">
            {Array.isArray(configDraft.a2a_peers) &&
            (configDraft.a2a_peers as A2APeerDraft[])
              .length > 0 ? (
              (configDraft.a2a_peers as A2APeerDraft[]).map(
                (peer, index) => (
                  <Card
                    key={`a2a-peer-${index}`}
                    className="p-3"
                  >
                    <div className="flex items-center justify-between gap-3">
                      <Text size="2" weight="medium">
                        {String(peer.name || "").trim() ||
                          `Peer #${index + 1}`}
                      </Text>
                      <Button
                        variant="soft"
                        color="red"
                        size="1"
                        onClick={() => removeA2APeer(index)}
                      >
                        Remove
                      </Button>
                    </div>
                    <div className="mt-3 grid grid-cols-1 gap-3">
                      <ConfigToggleCard
                        label={`a2a_peer_${index + 1}_enabled`}
                        description={
                          <>
                            Whether this peer can be
                            targeted by outbound delegation.
                          </>
                        }
                        checked={peer.enabled !== false}
                        onCheckedChange={(checked) =>
                          updateA2APeer(index, {
                            enabled: checked,
                          })
                        }
                        className={toggleCardClass}
                        style={toggleCardStyle}
                      />
                      <ConfigFieldCard
                        label={`a2a_peer_${index + 1}_name`}
                        description={
                          <>
                            Peer key used in{" "}
                            <code>a2a_send</code>, for
                            example <code>worker</code>.
                          </>
                        }
                      >
                        <TextField.Root
                          className="mt-2"
                          value={peer.name}
                          onChange={(e) =>
                            updateA2APeer(index, {
                              name: e.target.value,
                            })
                          }
                          placeholder="worker"
                        />
                      </ConfigFieldCard>
                      <ConfigFieldCard
                        label={`a2a_peer_${index + 1}_base_url`}
                        description={
                          <>
                            Remote base URL, for example{" "}
                            <code>
                              https://worker.example.com
                            </code>
                            .
                          </>
                        }
                      >
                        <TextField.Root
                          className="mt-2"
                          value={peer.base_url}
                          onChange={(e) =>
                            updateA2APeer(index, {
                              base_url: e.target.value,
                            })
                          }
                          placeholder="https://worker.example.com"
                        />
                      </ConfigFieldCard>
                      <ConfigFieldCard
                        label={`a2a_peer_${index + 1}_bearer_token`}
                        description={
                          <>
                            Optional outbound bearer token.
                            Leave blank to keep existing
                            token unchanged.
                          </>
                        }
                      >
                        <TextField.Root
                          className="mt-2"
                          value={peer.bearer_token}
                          onChange={(e) =>
                            updateA2APeer(index, {
                              bearer_token: e.target.value,
                            })
                          }
                          placeholder="shared-a2a-token"
                        />
                        {peer.has_bearer_token &&
                        !String(
                          peer.bearer_token || "",
                        ).trim() ? (
                          <Text
                            size="1"
                            color="gray"
                            className="mt-2 block"
                          >
                            Existing token is configured and
                            will be preserved.
                          </Text>
                        ) : null}
                      </ConfigFieldCard>
                      <ConfigFieldCard
                        label={`a2a_peer_${index + 1}_description`}
                        description={
                          <>
                            Optional description shown by{" "}
                            <code>a2a_list_peers</code>.
                          </>
                        }
                      >
                        <TextField.Root
                          className="mt-2"
                          value={peer.description}
                          onChange={(e) =>
                            updateA2APeer(index, {
                              description: e.target.value,
                            })
                          }
                          placeholder="Executes implementation tasks"
                        />
                      </ConfigFieldCard>
                      <ConfigFieldCard
                        label={`a2a_peer_${index + 1}_default_session_key`}
                        description={
                          <>
                            Optional default remote session
                            key.
                          </>
                        }
                      >
                        <TextField.Root
                          className="mt-2"
                          value={peer.default_session_key}
                          onChange={(e) =>
                            updateA2APeer(index, {
                              default_session_key:
                                e.target.value,
                            })
                          }
                          placeholder="a2a:worker"
                        />
                      </ConfigFieldCard>
                    </div>
                  </Card>
                ),
              )
            ) : (
              <Text size="1" color="gray">
                No peers configured yet.
              </Text>
            )}
            <Button
              variant="soft"
              onClick={() => addA2APeer()}
            >
              Add Peer
            </Button>
          </div>
        </ConfigFieldCard>
      </div>
    </div>
  );
}
