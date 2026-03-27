import React from "react";
import { Text, TextField } from "@radix-ui/themes";
import { ConfigFieldCard } from "../config-field-card";
import { ConfigToggleCard } from "../config-toggle-card";
import { useSettings } from "../../context/settings-context";
import {
  DEFAULT_CONFIG_VALUES,
} from "../../lib/constants";
import { normalizeWorkingDirIsolation } from "../../lib/config-helpers";

export function GeneralTab(): React.ReactElement {
  const {
    configDraft,
    setConfigField,
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
        General
      </Text>
      <Text size="1" color="gray" className="mt-1 block">
        Runtime defaults used across all channels.
      </Text>
      <Text size="1" color="gray" className="mt-2 block">
        working_dir_isolation: chat = isolated workspace per
        chat; shared = one shared workspace.
      </Text>
      <Text size="1" color="gray" className="mt-1 block">
        max_tokens / max_tool_iterations /
        max_document_size_mb / memory_token_budget control
        response budget and tool loop safety.
      </Text>
      <div className="mt-4 space-y-3">
        <ConfigFieldCard
          label="bot_username"
          description={
            <>
              Global default bot username. Channel-specific{" "}
              <code>
                channels.&lt;name&gt;.bot_username
              </code>{" "}
              overrides this.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(configDraft.bot_username || "")}
            onChange={(e) =>
              setConfigField("bot_username", e.target.value)
            }
            placeholder="bot"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="working_dir_isolation"
          description={
            <>
              Use <code>chat</code> for per-chat isolation,
              or <code>shared</code> for one shared
              workspace.
            </>
          }
        >
          <select
            className="mt-2 w-full rounded-md border border-[color:var(--mc-border-soft)] bg-transparent px-3 py-2 text-base text-[color:inherit] outline-none focus:border-[color:var(--mc-accent)]"
            value={normalizeWorkingDirIsolation(
              configDraft.working_dir_isolation ||
                DEFAULT_CONFIG_VALUES.working_dir_isolation,
            )}
            onChange={(e) =>
              setConfigField(
                "working_dir_isolation",
                e.target.value,
              )
            }
          >
            <option value="chat">
              chat (per-chat isolated workspace)
            </option>
            <option value="shared">
              shared (single shared workspace)
            </option>
          </select>
        </ConfigFieldCard>
        <ConfigFieldCard
          label="souls_dir"
          description={
            <>
              Directory used by SOUL picker and default SOUL
              path normalization.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(configDraft.souls_dir || "")}
            onChange={(e) =>
              setConfigField("souls_dir", e.target.value)
            }
            placeholder="~/.microclaw/souls"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="max_tokens"
          description={
            <>
              Maximum output tokens for one model response.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(
              configDraft.max_tokens ||
                DEFAULT_CONFIG_VALUES.max_tokens,
            )}
            onChange={(e) =>
              setConfigField("max_tokens", e.target.value)
            }
            placeholder="8192"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="max_tool_iterations"
          description={
            <>
              Upper bound for tool loop iterations in one
              request.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(
              configDraft.max_tool_iterations ||
                DEFAULT_CONFIG_VALUES.max_tool_iterations,
            )}
            onChange={(e) =>
              setConfigField(
                "max_tool_iterations",
                e.target.value,
              )
            }
            placeholder="100"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="max_document_size_mb"
          description={
            <>Maximum uploaded file size in MB.</>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(
              configDraft.max_document_size_mb ||
                DEFAULT_CONFIG_VALUES.max_document_size_mb,
            )}
            onChange={(e) =>
              setConfigField(
                "max_document_size_mb",
                e.target.value,
              )
            }
            placeholder="100"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="memory_token_budget"
          description={
            <>
              Estimated token budget for injecting
              structured memories into the system prompt.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            type="number"
            value={String(
              configDraft.memory_token_budget ||
                DEFAULT_CONFIG_VALUES.memory_token_budget,
            )}
            onChange={(e) =>
              setConfigField(
                "memory_token_budget",
                e.target.value,
              )
            }
            placeholder="1500"
          />
        </ConfigFieldCard>
      </div>
      <div className="mt-4 grid grid-cols-1 gap-3">
        <ConfigToggleCard
          label="high_risk_tool_user_confirmation_required"
          description={
            <>
              Require explicit confirmation before running
              high-risk tools (for example <code>bash</code>
              ).
            </>
          }
          checked={
            configDraft.high_risk_tool_user_confirmation_required !==
            false
          }
          onCheckedChange={(checked) =>
            setConfigField(
              "high_risk_tool_user_confirmation_required",
              checked,
            )
          }
          className={toggleCardClass}
          style={toggleCardStyle}
        />
        <ConfigToggleCard
          label="show_thinking"
          description={
            <>
              Show intermediate reasoning text in responses.
            </>
          }
          checked={Boolean(configDraft.show_thinking)}
          onCheckedChange={(checked) =>
            setConfigField("show_thinking", checked)
          }
          className={toggleCardClass}
          style={toggleCardStyle}
        />
        <ConfigToggleCard
          label="web_enabled"
          description={
            <>Enable built-in Web UI and API endpoint.</>
          }
          checked={Boolean(configDraft.web_enabled)}
          onCheckedChange={(checked) =>
            setConfigField("web_enabled", checked)
          }
          className={toggleCardClass}
          style={toggleCardStyle}
        />
        <ConfigToggleCard
          label="reflector_enabled"
          description={
            <>
              Periodically extract structured memories from
              conversations in the background.
            </>
          }
          checked={configDraft.reflector_enabled !== false}
          onCheckedChange={(checked) =>
            setConfigField("reflector_enabled", checked)
          }
          className={toggleCardClass}
          style={toggleCardStyle}
        />
      </div>
      <div className="mt-4 space-y-3">
        <ConfigFieldCard
          label="reflector_interval_mins"
          description={
            <>
              How often (in minutes) the memory reflector
              runs. Requires restart.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            type="number"
            value={String(
              configDraft.reflector_interval_mins ??
                DEFAULT_CONFIG_VALUES.reflector_interval_mins,
            )}
            onChange={(e) =>
              setConfigField(
                "reflector_interval_mins",
                e.target.value,
              )
            }
            placeholder="15"
          />
        </ConfigFieldCard>
      </div>
    </div>
  );
}
