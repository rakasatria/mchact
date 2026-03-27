import type { ChatModelRunOptions, ThreadMessageLike } from "@assistant-ui/react";
import type { BackendMessage } from "./types";
import type { SessionItem } from "../types";

export function writeSessionToUrl(sessionKey: string): void {
  if (typeof window === "undefined") return;
  const url = new URL(window.location.href);
  url.searchParams.set("session", sessionKey);
  window.history.replaceState(null, "", url.toString());
}

export function readSessionFromUrl(): string {
  if (typeof window === "undefined") return "";
  const url = new URL(window.location.href);
  return url.searchParams.get("session")?.trim() || "";
}

export function makeSessionKey(): string {
  return `session-${new Date()
    .toISOString()
    .replace(/[-:TZ.]/g, "")
    .slice(0, 14)}`;
}

export function pickLatestSessionKey(items: SessionItem[]): string {
  if (items.length === 0) return makeSessionKey();

  const parsed = items
    .map((item) => ({ item, ts: Date.parse(item.last_message_time || "") }))
    .filter((v) => Number.isFinite(v.ts));

  if (parsed.length > 0) {
    parsed.sort((a, b) => b.ts - a.ts);
    return parsed[0]?.item.session_key || makeSessionKey();
  }

  return items[0]?.session_key || makeSessionKey();
}

export function readBootstrapTokenFromHash(): string {
  if (typeof window === "undefined") return "";
  const raw = window.location.hash.startsWith("#")
    ? window.location.hash.slice(1)
    : window.location.hash;
  const params = new URLSearchParams(raw);
  return params.get("bootstrap")?.trim() || "";
}

export function clearBootstrapTokenFromHash(): void {
  if (typeof window === "undefined") return;
  const raw = window.location.hash.startsWith("#")
    ? window.location.hash.slice(1)
    : window.location.hash;
  const params = new URLSearchParams(raw);
  if (!params.has("bootstrap")) return;
  params.delete("bootstrap");
  const next = params.toString();
  window.location.hash = next ? `#${next}` : "";
}

export function generatePassword(): string {
  if (
    typeof crypto !== "undefined" &&
    typeof crypto.randomUUID === "function"
  ) {
    const raw = crypto.randomUUID().replace(/-/g, "");
    return `mc-${raw.slice(0, 6)}-${raw.slice(6, 12)}!`;
  }
  const fallback = Math.random().toString(36).slice(2, 14);
  return `mc-${fallback.slice(0, 6)}-${fallback.slice(6, 12)}!`;
}

export function extractLatestUserText(
  messages: readonly ChatModelRunOptions["messages"][number][],
): string {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i];
    if (message.role !== "user") continue;

    const text = message.content
      .map((part) => {
        if (part.type === "text") return part.text;
        return "";
      })
      .join("\n")
      .trim();

    if (text.length > 0) return text;
  }
  return "";
}

export function mapBackendHistory(messages: BackendMessage[]): ThreadMessageLike[] {
  return messages.map((item, index) => ({
    id: item.id || `history-${index}`,
    role: item.is_from_bot ? "assistant" : "user",
    content: item.content || "",
    createdAt: item.timestamp ? new Date(item.timestamp) : new Date(),
    ...(item.attachments && item.attachments.length > 0
      ? { metadata: { attachments: item.attachments } }
      : {}),
  }));
}
