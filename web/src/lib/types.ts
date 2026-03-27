export type ThinkExtraction = {
  visibleText: string;
  thinkSegments: string[];
};

export type ConfigPayload = Record<string, unknown>;

export type StreamEvent = {
  event: string;
  payload: Record<string, unknown>;
};

export type AuthStatusResponse = {
  ok?: boolean;
  authenticated?: boolean;
  has_password?: boolean;
  using_default_password?: boolean;
};

export type HealthResponse = {
  version?: string;
};

export type BackendMessage = {
  id?: string;
  sender_name?: string;
  content?: string;
  is_from_bot?: boolean;
  timestamp?: string;
};

export type ConfigWarning = {
  code?: string;
  severity?: string;
  message?: string;
};

export type ExecutionPolicyItem = {
  tool?: string;
  risk?: string;
  policy?: string;
};

export type MountAllowlistStatus = {
  path?: string;
  exists?: boolean;
  has_entries?: boolean;
};

export type SecurityPosture = {
  sandbox_mode?: "off" | "all" | string;
  sandbox_runtime_available?: boolean;
  sandbox_backend?: string;
  sandbox_require_runtime?: boolean;
  execution_policies?: ExecutionPolicyItem[];
  mount_allowlist?: MountAllowlistStatus | null;
};

export type ConfigSelfCheck = {
  ok?: boolean;
  risk_level?: "none" | "medium" | "high" | string;
  warning_count?: number;
  warnings?: ConfigWarning[];
  security_posture?: SecurityPosture;
};

export type A2APeerDraft = {
  name: string;
  enabled: boolean;
  base_url: string;
  bearer_token: string;
  has_bearer_token?: boolean;
  description: string;
  default_session_key: string;
};

export type ProviderProfileDraft = {
  id: string;
  provider: string;
  api_key: string;
  llm_base_url: string;
  llm_user_agent: string;
  default_model: string;
  show_thinking: boolean;
};

export type ToolStartPayload = {
  tool_use_id: string;
  name: string;
  input?: unknown;
};

export type ToolResultPayload = {
  tool_use_id: string;
  name: string;
  is_error?: boolean;
  output?: unknown;
  duration_ms?: number;
  bytes?: number;
  status_code?: number;
  error_type?: string;
};

export type Appearance = "dark" | "light";
export type UiTheme =
  | "green"
  | "blue"
  | "slate"
  | "amber"
  | "violet"
  | "rose"
  | "cyan"
  | "teal"
  | "orange"
  | "indigo";

export interface DynChannelField {
  /** YAML key inside channels.<name>, e.g. "bot_token" */
  yamlKey: string;
  /** Label shown in the settings panel */
  label: string;
  /** Input placeholder */
  placeholder: string;
  /** Description shown in ConfigFieldCard */
  description: string;
  /** If true, field value is a secret (not pre-filled from server config) */
  secret: boolean;
  /** Value type for payload encoding */
  valueType?: "string" | "bool" | "number";
}

export interface DynChannelDef {
  /** Channel name, e.g. "slack" */
  name: string;
  /** Display title for the tab header */
  title: string;
  /** Emoji icon for the tab trigger */
  icon: string;
  /** Setup steps shown in ConfigStepsCard */
  steps: string[];
  /** Hint text below the steps */
  hint: string;
  /** Optional channel-level config fields */
  channelFields?: DynChannelField[];
  /** Config fields */
  fields: DynChannelField[];
}
