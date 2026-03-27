import { useEffect, useState } from "react";
import {
  Badge,
  Button,
  Callout,
  Card,
  Flex,
  Text,
  TextField,
} from "@radix-ui/themes";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import {
  faServer,
  faWrench,
  faCircleCheck,
  faChevronRight,
  faChevronDown,
  faPlug,
  faCircleExclamation,
  faSpinner,
  faPlus,
  faTrash,
  faPen,
  faFloppyDisk,
  faXmark,
  faTerminal,
  faGlobe,
} from "@fortawesome/free-solid-svg-icons";
import { api } from "../lib/api";
import { ConfigFieldCard } from "./config-field-card";

// --- Types ---

type McpToolStatus = {
  qualified_name: string;
  original_name: string;
  description: string;
};

type McpServerStatus = {
  name: string;
  tools: McpToolStatus[];
};

type McpResponse = {
  ok: boolean;
  servers: McpServerStatus[];
};

type McpConfigResponse = {
  ok: boolean;
  config: {
    mcpServers: Record<string, McpServerConfigDraft>;
    defaultProtocolVersion?: string;
  };
  path: string;
};

type McpServerConfigDraft = {
  transport?: string;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  endpoint?: string;
  headers?: Record<string, string>;
  request_timeout_secs?: number;
  max_retries?: number;
  [key: string]: unknown;
};

type EditingServer = {
  name: string;
  transport: string;
  command: string;
  args: string;
  env: string;
  endpoint: string;
  headers: string;
};

// --- Helpers ---

function emptyServer(): EditingServer {
  return {
    name: "",
    transport: "stdio",
    command: "",
    args: "",
    env: "",
    endpoint: "",
    headers: "",
  };
}

function draftToEditing(
  name: string,
  draft: McpServerConfigDraft,
): EditingServer {
  return {
    name,
    transport: draft.transport || "stdio",
    command: draft.command || "",
    args: (draft.args || []).join(", "),
    env: Object.entries(draft.env || {})
      .map(([k, v]) => `${k}=${v}`)
      .join("\n"),
    endpoint: draft.endpoint || "",
    headers: Object.entries(draft.headers || {})
      .map(([k, v]) => `${k}: ${v}`)
      .join("\n"),
  };
}

function editingToDraft(editing: EditingServer): McpServerConfigDraft {
  const draft: McpServerConfigDraft = { transport: editing.transport };
  if (editing.transport === "stdio") {
    draft.command = editing.command.trim();
    const args = editing.args.trim();
    if (args)
      draft.args = args
        .split(",")
        .map((a) => a.trim())
        .filter(Boolean);
    const env = editing.env.trim();
    if (env) {
      draft.env = Object.fromEntries(
        env
          .split("\n")
          .map((line) => {
            const eq = line.indexOf("=");
            return eq > 0
              ? [line.slice(0, eq).trim(), line.slice(eq + 1).trim()]
              : [line.trim(), ""];
          })
          .filter(([k]) => k),
      );
    }
  } else {
    draft.endpoint = editing.endpoint.trim();
    const headers = editing.headers.trim();
    if (headers) {
      draft.headers = Object.fromEntries(
        headers
          .split("\n")
          .map((line) => {
            const colon = line.indexOf(":");
            return colon > 0
              ? [line.slice(0, colon).trim(), line.slice(colon + 1).trim()]
              : [line.trim(), ""];
          })
          .filter(([k]) => k),
      );
    }
  }
  return draft;
}

// --- Sub-components ---

function McpSummaryBadges({
  serverCount,
  toolCount,
}: {
  serverCount: number;
  toolCount: number;
}) {
  return (
    <Flex gap="2" className="mb-1">
      <Badge size="1" color="green">
        <FontAwesomeIcon icon={faServer} className="mr-1" />
        {serverCount} server{serverCount !== 1 ? "s" : ""}
      </Badge>
      <Badge size="1" color="blue">
        <FontAwesomeIcon icon={faWrench} className="mr-1" />
        {toolCount} tool{toolCount !== 1 ? "s" : ""}
      </Badge>
    </Flex>
  );
}

function McpToolItem({ tool }: { tool: McpToolStatus }) {
  return (
    <Flex direction="column" gap="1" className="py-1">
      <Flex align="center" gap="2">
        <FontAwesomeIcon icon={faWrench} size="xs" style={{ opacity: 0.4 }} />
        <Text size="2" weight="medium" style={{ fontFamily: "monospace" }}>
          {tool.original_name}
        </Text>
      </Flex>
      {tool.description && (
        <Text size="1" color="gray" className="line-clamp-2 pl-5">
          {tool.description}
        </Text>
      )}
    </Flex>
  );
}

function McpServerCard({
  server,
  expanded,
  onToggle,
  onEdit,
  onDelete,
}: {
  server: McpServerStatus;
  expanded: boolean;
  onToggle: () => void;
  onEdit: () => void;
  onDelete: () => void;
}) {
  return (
    <Card variant="surface" className="p-3">
      <Flex
        justify="between"
        align="center"
        style={{ cursor: "pointer" }}
        onClick={onToggle}
      >
        <Flex align="center" gap="2">
          <FontAwesomeIcon
            icon={expanded ? faChevronDown : faChevronRight}
            size="xs"
            style={{ opacity: 0.5, width: 12 }}
          />
          <FontAwesomeIcon icon={faPlug} size="sm" style={{ opacity: 0.7 }} />
          <Text weight="bold" size="2">
            {server.name}
          </Text>
          <Badge size="1" color="gray">
            {server.tools.length} tool{server.tools.length !== 1 ? "s" : ""}
          </Badge>
        </Flex>
        <Flex align="center" gap="2">
          <Badge size="1" color="green" variant="soft">
            <FontAwesomeIcon icon={faCircleCheck} className="mr-1" />
            Connected
          </Badge>
          <Button
            size="1"
            variant="ghost"
            onClick={(e) => {
              e.stopPropagation();
              onEdit();
            }}
          >
            <FontAwesomeIcon icon={faPen} />
          </Button>
          <Button
            size="1"
            variant="ghost"
            color="red"
            onClick={(e) => {
              e.stopPropagation();
              onDelete();
            }}
          >
            <FontAwesomeIcon icon={faTrash} />
          </Button>
        </Flex>
      </Flex>

      {expanded && (
        <div
          className="mt-3 flex flex-col gap-1 pl-4 border-l-2"
          style={{ borderColor: "var(--mc-border-soft)" }}
        >
          {server.tools.map((tool) => (
            <McpToolItem key={tool.qualified_name} tool={tool} />
          ))}
        </div>
      )}
    </Card>
  );
}

function ConfiguredServerCard({
  name,
  config,
  isConnected,
  onEdit,
  onDelete,
}: {
  name: string;
  config: McpServerConfigDraft;
  isConnected: boolean;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const transport = config.transport || "stdio";
  return (
    <Card variant="surface" className="p-3">
      <Flex justify="between" align="center">
        <Flex align="center" gap="2">
          <FontAwesomeIcon
            icon={transport === "stdio" ? faTerminal : faGlobe}
            size="sm"
            style={{ opacity: 0.7 }}
          />
          <Text weight="bold" size="2">
            {name}
          </Text>
          <Badge size="1" color="gray">
            {transport}
          </Badge>
          {transport === "stdio" && config.command && (
            <Text size="1" color="gray" style={{ fontFamily: "monospace" }}>
              {config.command}
            </Text>
          )}
          {transport === "streamable_http" && config.endpoint && (
            <Text
              size="1"
              color="gray"
              style={{ fontFamily: "monospace" }}
              className="truncate max-w-[200px]"
            >
              {config.endpoint}
            </Text>
          )}
        </Flex>
        <Flex align="center" gap="2">
          {isConnected ? (
            <Badge size="1" color="green" variant="soft">
              <FontAwesomeIcon icon={faCircleCheck} className="mr-1" />
              Connected
            </Badge>
          ) : (
            <Badge size="1" color="orange" variant="soft">
              Pending restart
            </Badge>
          )}
          <Button size="1" variant="ghost" onClick={onEdit}>
            <FontAwesomeIcon icon={faPen} />
          </Button>
          <Button size="1" variant="ghost" color="red" onClick={onDelete}>
            <FontAwesomeIcon icon={faTrash} />
          </Button>
        </Flex>
      </Flex>
    </Card>
  );
}

function editingToJson(name: string, editing: EditingServer): string {
  const draft = editingToDraft(editing);
  return JSON.stringify({ [name || "my-server"]: draft }, null, 2);
}

function jsonToEditing(
  json: string,
): { name: string; editing: EditingServer } | string {
  try {
    const parsed = JSON.parse(json);
    if (
      typeof parsed !== "object" ||
      parsed === null ||
      Array.isArray(parsed)
    ) {
      return 'JSON must be an object like { "server-name": { ... } }';
    }
    const entries = Object.entries(parsed);
    if (entries.length !== 1) {
      return "JSON must contain exactly one server entry";
    }
    const [name, config] = entries[0];
    if (typeof config !== "object" || config === null) {
      return "Server config must be an object";
    }
    return {
      name,
      editing: draftToEditing(name, config as McpServerConfigDraft),
    };
  } catch (e) {
    return e instanceof Error ? e.message : "Invalid JSON";
  }
}

function ServerForm({
  editing,
  onChange,
  onSave,
  onCancel,
  isNew,
  existingConfig,
}: {
  editing: EditingServer;
  onChange: (next: EditingServer) => void;
  onSave: () => void;
  onCancel: () => void;
  isNew: boolean;
  existingConfig?: McpServerConfigDraft;
}) {
  const [mode, setMode] = useState<"form" | "json">("form");
  const [jsonText, setJsonText] = useState(() =>
    isNew ? editingToJson("", editing) : editingToJson(editing.name, editing),
  );
  const [jsonError, setJsonError] = useState("");

  const set = (field: keyof EditingServer, value: string) => {
    const next = { ...editing, [field]: value };
    onChange(next);
    setJsonText(editingToJson(next.name, next));
  };

  const switchToJson = () => {
    setJsonText(editingToJson(editing.name, editing));
    setJsonError("");
    setMode("json");
  };

  const switchToForm = () => {
    const result = jsonToEditing(jsonText);
    if (typeof result === "string") {
      setJsonError(result);
      return;
    }
    onChange(result.editing);
    if (isNew) onChange({ ...result.editing, name: result.name });
    setJsonError("");
    setMode("form");
  };

  const handleJsonSave = () => {
    const result = jsonToEditing(jsonText);
    if (typeof result === "string") {
      setJsonError(result);
      return;
    }
    if (isNew) onChange({ ...result.editing, name: result.name });
    else onChange(result.editing);
    setJsonError("");
    onSave();
  };

  return (
    <Card variant="surface" className="p-4">
      <Flex justify="between" align="center" className="mb-3">
        <Text size="2" weight="bold">
          {isNew ? "Add MCP Server" : `Edit: ${editing.name}`}
        </Text>
        <Flex gap="1">
          <Button
            size="1"
            variant={mode === "form" ? "solid" : "soft"}
            onClick={() => (mode === "json" ? switchToForm() : undefined)}
          >
            Form
          </Button>
          <Button
            size="1"
            variant={mode === "json" ? "solid" : "soft"}
            onClick={() => (mode === "form" ? switchToJson() : undefined)}
          >
            JSON
          </Button>
        </Flex>
      </Flex>

      {mode === "form" ? (
        <div className="flex flex-col gap-3">
          <div>
            <Text size="1" weight="medium" className="mb-1 block">
              Server Name
            </Text>
            <TextField.Root
              value={editing.name}
              onChange={(e) => set("name", e.target.value)}
              placeholder="my-server"
              disabled={!isNew}
            />
          </div>
          <div>
            <Text size="1" weight="medium" className="mb-1 block">
              Transport
            </Text>
            <select
              className="w-full rounded-md border border-[color:var(--mc-border-soft)] bg-transparent px-3 py-2 text-sm text-[color:inherit] outline-none"
              value={editing.transport}
              onChange={(e) => set("transport", e.target.value)}
            >
              <option value="stdio">stdio (local command)</option>
              <option value="streamable_http">
                streamable_http (remote URL)
              </option>
            </select>
          </div>

          {editing.transport === "stdio" ? (
            <>
              <div>
                <Text size="1" weight="medium" className="mb-1 block">
                  Command
                </Text>
                <TextField.Root
                  value={editing.command}
                  onChange={(e) => set("command", e.target.value)}
                  placeholder="npx"
                />
              </div>
              <div>
                <Text size="1" weight="medium" className="mb-1 block">
                  Args (comma-separated)
                </Text>
                <TextField.Root
                  value={editing.args}
                  onChange={(e) => set("args", e.target.value)}
                  placeholder="-y, @modelcontextprotocol/server-filesystem, /path"
                />
              </div>
              <div>
                <Text size="1" weight="medium" className="mb-1 block">
                  Environment (KEY=VALUE per line)
                </Text>
                <textarea
                  className="w-full rounded-md border border-[color:var(--mc-border-soft)] bg-transparent px-3 py-2 text-sm text-[color:inherit] outline-none font-mono"
                  rows={2}
                  value={editing.env}
                  onChange={(e) => set("env", e.target.value)}
                  placeholder="NODE_ENV=production"
                />
              </div>
            </>
          ) : (
            <>
              <div>
                <Text size="1" weight="medium" className="mb-1 block">
                  Endpoint URL
                </Text>
                <TextField.Root
                  value={editing.endpoint}
                  onChange={(e) => set("endpoint", e.target.value)}
                  placeholder="https://my-mcp-server.com/mcp"
                />
              </div>
              <div>
                <Text size="1" weight="medium" className="mb-1 block">
                  Headers (Name: Value per line)
                </Text>
                <textarea
                  className="w-full rounded-md border border-[color:var(--mc-border-soft)] bg-transparent px-3 py-2 text-sm text-[color:inherit] outline-none font-mono"
                  rows={2}
                  value={editing.headers}
                  onChange={(e) => set("headers", e.target.value)}
                  placeholder="Authorization: Bearer sk-xxx"
                />
              </div>
            </>
          )}
        </div>
      ) : (
        <div className="flex flex-col gap-2">
          <Text size="1" color="gray">
            Paste or edit the server config as JSON. Format:{" "}
            <code>{'{ "name": { "transport": "stdio", ... } }'}</code>
          </Text>
          {jsonError && (
            <Callout.Root color="red" size="1" variant="soft">
              <Callout.Text>
                <FontAwesomeIcon icon={faCircleExclamation} className="mr-1" />
                {jsonError}
              </Callout.Text>
            </Callout.Root>
          )}
          <textarea
            className="w-full rounded-md border border-[color:var(--mc-border-soft)] bg-transparent px-3 py-2 text-sm text-[color:inherit] outline-none font-mono"
            rows={10}
            value={jsonText}
            onChange={(e) => {
              setJsonText(e.target.value);
              setJsonError("");
            }}
            spellCheck={false}
          />
        </div>
      )}

      <Flex gap="2" justify="end" className="mt-3">
        <Button size="1" variant="soft" onClick={onCancel}>
          <FontAwesomeIcon icon={faXmark} className="mr-1" />
          Cancel
        </Button>
        <Button size="1" onClick={mode === "json" ? handleJsonSave : onSave}>
          <FontAwesomeIcon icon={faFloppyDisk} className="mr-1" />
          {isNew ? "Add" : "Save"}
        </Button>
      </Flex>
    </Card>
  );
}

// --- Main component ---

export function McpSettings() {
  const [servers, setServers] = useState<McpServerStatus[]>([]);
  const [configServers, setConfigServers] = useState<
    Record<string, McpServerConfigDraft>
  >({});
  const [configPath, setConfigPath] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [saveStatus, setSaveStatus] = useState("");
  const [expandedServers, setExpandedServers] = useState<Set<string>>(
    new Set(),
  );
  const [editing, setEditing] = useState<EditingServer | null>(null);
  const [editingIsNew, setEditingIsNew] = useState(false);

  const loadData = async () => {
    setLoading(true);
    setError("");
    try {
      const [liveRes, configRes] = await Promise.all([
        api<McpResponse>("/api/mcp"),
        api<McpConfigResponse>("/api/mcp/config"),
      ]);
      setServers(liveRes.servers);
      setConfigServers(configRes.config?.mcpServers || {});
      setConfigPath(configRes.path || "");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void loadData();
  }, []);

  const toggleExpanded = (name: string) => {
    setExpandedServers((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  };

  const connectedNames = new Set(servers.map((s) => s.name));
  const allServerNames = Array.from(
    new Set([...servers.map((s) => s.name), ...Object.keys(configServers)]),
  ).sort();

  const totalTools = servers.reduce((sum, s) => sum + s.tools.length, 0);

  const saveConfig = async (
    nextServers: Record<string, McpServerConfigDraft>,
  ) => {
    setSaveStatus("");
    setError("");
    try {
      const res = await api<{ ok: boolean; message?: string }>(
        "/api/mcp/config",
        {
          method: "PUT",
          body: JSON.stringify({ config: { mcpServers: nextServers } }),
        },
      );
      setConfigServers(nextServers);
      setSaveStatus(res.message || "Saved. Restart to apply.");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleStartAdd = () => {
    setEditing(emptyServer());
    setEditingIsNew(true);
    setSaveStatus("");
  };

  const handleStartEdit = (name: string) => {
    const draft = configServers[name] || {};
    setEditing(draftToEditing(name, draft));
    setEditingIsNew(false);
    setSaveStatus("");
  };

  const handleDelete = async (name: string) => {
    const next = { ...configServers };
    delete next[name];
    await saveConfig(next);
  };

  const handleSaveForm = async () => {
    if (!editing) return;
    const name = editing.name.trim();
    if (!name) {
      setError("Server name is required");
      return;
    }
    if (editingIsNew && configServers[name]) {
      setError(`Server '${name}' already exists`);
      return;
    }

    const draft = editingToDraft(editing);
    const next = { ...configServers, [name]: draft };
    await saveConfig(next);
    setEditing(null);
  };

  return (
    <div className="flex flex-col gap-4 h-full">
      {error && (
        <Callout.Root color="red" size="1" variant="soft">
          <Callout.Text>
            <FontAwesomeIcon icon={faCircleExclamation} className="mr-1" />
            {error}
          </Callout.Text>
        </Callout.Root>
      )}
      {saveStatus && (
        <Callout.Root color="green" size="1" variant="soft">
          <Callout.Text>
            <FontAwesomeIcon icon={faCircleCheck} className="mr-1" />
            {saveStatus}
          </Callout.Text>
        </Callout.Root>
      )}

      <ConfigFieldCard
        label="MCP Servers"
        description={
          <>
            Manage Model Context Protocol servers.
            {configPath && (
              <>
                {" "}
                Config: <code>{configPath}</code>
              </>
            )}
          </>
        }
      >
        <div className="flex flex-col gap-2 mt-2">
          {loading && (
            <Text size="1" color="gray">
              <FontAwesomeIcon icon={faSpinner} spin className="mr-1" />
              Loading...
            </Text>
          )}

          {!loading && allServerNames.length === 0 && !editing && (
            <Text size="1" color="gray">
              No MCP servers configured. Click "Add Server" to get started.
            </Text>
          )}

          {!loading && allServerNames.length > 0 && (
            <McpSummaryBadges
              serverCount={allServerNames.length}
              toolCount={totalTools}
            />
          )}

          {allServerNames.map((name) => {
            const liveServer = servers.find((s) => s.name === name);
            const isConnected = connectedNames.has(name);

            if (liveServer) {
              return (
                <McpServerCard
                  key={name}
                  server={liveServer}
                  expanded={expandedServers.has(name)}
                  onToggle={() => toggleExpanded(name)}
                  onEdit={() => handleStartEdit(name)}
                  onDelete={() => void handleDelete(name)}
                />
              );
            }

            return (
              <ConfiguredServerCard
                key={name}
                name={name}
                config={configServers[name] || {}}
                isConnected={isConnected}
                onEdit={() => handleStartEdit(name)}
                onDelete={() => void handleDelete(name)}
              />
            );
          })}

          {editing && (
            <ServerForm
              editing={editing}
              onChange={setEditing}
              onSave={() => void handleSaveForm()}
              onCancel={() => setEditing(null)}
              isNew={editingIsNew}
            />
          )}

          {!editing && (
            <Button
              size="1"
              variant="soft"
              onClick={handleStartAdd}
              className="mt-1 self-start"
            >
              <FontAwesomeIcon icon={faPlus} className="mr-1" />
              Add Server
            </Button>
          )}
        </div>
      </ConfigFieldCard>
    </div>
  );
}
