import React from "react";
import { Badge, Callout, Card, Text, TextField } from "@radix-ui/themes";
import { ConfigFieldCard } from "../config-field-card";
import { ConfigStepsCard } from "../config-steps-card";
import { useSettings } from "../../context/settings-context";
import { DEFAULT_CONFIG_VALUES } from "../../lib/constants";
import { warningDocUrl } from "../../lib/config-helpers";

export function WebTab(): React.ReactElement {
  const {
    configDraft,
    setConfigField,
    configSelfCheck,
    sectionCardClass,
    sectionCardStyle,
  } = useSettings();

  return (
    <div
      className={sectionCardClass}
      style={sectionCardStyle}
    >
      <Text size="3" weight="bold">
        Web
      </Text>
      <ConfigStepsCard
        steps={[
          <>
            Keep <code>web_enabled</code> on for local UI
            access.
          </>,
          <>
            Use <code>127.0.0.1</code> for local-only host,
            or set LAN host explicitly.
          </>,
          <>
            Choose web port (default <code>10961</code>).
          </>,
        ]}
      />
      <Text size="1" color="gray" className="mt-3 block">
        For local only, keep host as 127.0.0.1. Use 0.0.0.0
        only behind trusted network controls.
      </Text>
      <div className="mt-4 space-y-3">
        <ConfigFieldCard
          label="web_host"
          description={
            <>
              Use <code>127.0.0.1</code> for local-only. Use{" "}
              <code>0.0.0.0</code> only when intentionally
              exposing on LAN.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(
              configDraft.web_host ||
                DEFAULT_CONFIG_VALUES.web_host,
            )}
            onChange={(e) =>
              setConfigField("web_host", e.target.value)
            }
            placeholder="127.0.0.1"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="web_port"
          description={
            <>HTTP port for Web UI and API endpoint.</>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(
              configDraft.web_port ||
                DEFAULT_CONFIG_VALUES.web_port,
            )}
            onChange={(e) =>
              setConfigField("web_port", e.target.value)
            }
            placeholder="10961"
          />
        </ConfigFieldCard>
        <ConfigFieldCard
          label="web_bot_username"
          description={
            <>
              Optional Web-specific bot username override.
            </>
          }
        >
          <TextField.Root
            className="mt-2"
            value={String(
              configDraft.web_bot_username || "",
            )}
            onChange={(e) =>
              setConfigField(
                "web_bot_username",
                e.target.value,
              )
            }
            placeholder="web_bot_name"
          />
        </ConfigFieldCard>
      </div>
      {Array.isArray(configSelfCheck?.warnings) &&
      configSelfCheck!.warnings!.length > 0 ? (
        <details className="mt-4">
          <summary className="cursor-pointer text-sm text-[color:var(--gray-11)]">
            Critical config warnings (
            {configSelfCheck!.warnings!.length})
          </summary>
          <Card className="mt-2 p-3">
            <Text size="2" weight="bold">
              Critical Config Warnings
            </Text>
            <div className="mt-2 space-y-2">
              {configSelfCheck!.warnings!.map((w, idx) => (
                <Callout.Root
                  key={`${w.code || "warning"}-${idx}`}
                  color={
                    w.severity === "high" ? "red" : "orange"
                  }
                  size="1"
                  variant="soft"
                >
                  <Callout.Text>
                    [{String(w.severity || "unknown")}]{" "}
                    {String(w.code || "warning")}:{" "}
                    {String(w.message || "")}{" "}
                    <a
                      href={warningDocUrl(w.code)}
                      target="_blank"
                      rel="noreferrer"
                      className="underline"
                    >
                      Docs
                    </a>
                  </Callout.Text>
                </Callout.Root>
              ))}
            </div>
          </Card>
        </details>
      ) : null}
    </div>
  );
}
