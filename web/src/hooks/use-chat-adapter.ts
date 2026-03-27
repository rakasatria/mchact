import { useMemo } from "react";
import type {
  ReadonlyJSONObject,
  ReadonlyJSONValue,
} from "assistant-stream/utils";
import type {
  ChatModelAdapter,
  ChatModelRunResult,
} from "@assistant-ui/react";

import { api, makeHeaders, ApiError } from "../lib/api";
import type { ToolStartPayload, ToolResultPayload } from "../lib/types";
import { toJsonObject } from "../lib/text-processing";
import { extractLatestUserText } from "../lib/session-utils";
import { parseSseFrames } from "../lib/sse-parser";

export type UseChatAdapterDeps = {
  sessionKey: string;
  selectedSessionReadOnly: boolean;
  isUnauthorizedError: (err: unknown) => boolean;
  lockForAuth: (message?: string) => void;
  loadSessions: () => Promise<void>;
  loadHistory: (target?: string) => Promise<void>;
  setSending: (v: boolean) => void;
  setStatusText: (v: string) => void;
  setReplayNotice: (v: string) => void;
  setError: (v: string) => void;
};

export function useChatAdapter(deps: UseChatAdapterDeps): ChatModelAdapter {
  const {
    sessionKey,
    selectedSessionReadOnly,
    isUnauthorizedError,
    lockForAuth,
    loadSessions,
    loadHistory,
    setSending,
    setStatusText,
    setReplayNotice,
    setError,
  } = deps;

  const adapter = useMemo<ChatModelAdapter>(
    () => ({
      run: async function* (options): AsyncGenerator<ChatModelRunResult, void> {
        const userText = extractLatestUserText(options.messages);
        if (!userText) return;

        setSending(true);
        setStatusText("Sending...");
        setReplayNotice("");
        setError("");

        try {
          if (selectedSessionReadOnly) {
            setStatusText("Read-only channel");
            throw new Error(
              "This channel is read-only in Web UI. Send messages from the original channel.",
            );
          }

          const sendResponse = await api<{ run_id?: string }>(
            "/api/send_stream",
            {
              method: "POST",
              body: JSON.stringify({
                session_key: sessionKey,
                sender_name: "web-user",
                message: userText,
              }),
              signal: options.abortSignal,
            },
          );

          const runId = sendResponse.run_id;
          if (!runId) {
            throw new Error("missing run_id");
          }

          const query = new URLSearchParams({ run_id: runId });
          const streamResponse = await fetch(
            `/api/stream?${query.toString()}`,
            {
              method: "GET",
              headers: makeHeaders(),
              credentials: "same-origin",
              cache: "no-store",
              signal: options.abortSignal,
            },
          );

          if (!streamResponse.ok) {
            const text = await streamResponse.text().catch(() => "");
            throw new ApiError(
              text || `HTTP ${streamResponse.status}`,
              streamResponse.status,
            );
          }

          let assistantText = "";
          const toolState = new Map<
            string,
            {
              name: string;
              args: ReadonlyJSONObject;
              result?: ReadonlyJSONValue;
              isError?: boolean;
            }
          >();

          const makeContent = () => {
            const toolParts = Array.from(toolState.entries()).map(
              ([toolCallId, tool]) => ({
                type: "tool-call" as const,
                toolCallId,
                toolName: tool.name,
                args: tool.args,
                argsText: JSON.stringify(tool.args),
                ...(tool.result ? { result: tool.result } : {}),
                ...(tool.isError !== undefined
                  ? { isError: tool.isError }
                  : {}),
              }),
            );

            return [
              ...(assistantText
                ? [{ type: "text" as const, text: assistantText }]
                : []),
              ...toolParts,
            ];
          };

          for await (const event of parseSseFrames(
            streamResponse,
            options.abortSignal,
          )) {
            const data = event.payload;

            if (event.event === "replay_meta") {
              if (data.replay_truncated === true) {
                const oldest =
                  typeof data.oldest_event_id === "number"
                    ? data.oldest_event_id
                    : null;
                const message =
                  oldest !== null
                    ? `Stream history was truncated. Recovery resumed from event #${oldest}.`
                    : "Stream history was truncated. Recovery resumed from the earliest available event.";
                setReplayNotice(message);
              }
              continue;
            }

            if (event.event === "status") {
              const message =
                typeof data.message === "string" ? data.message : "";
              if (message) setStatusText(message);
              continue;
            }

            if (event.event === "tool_start") {
              const payload = data as ToolStartPayload;
              if (!payload.tool_use_id || !payload.name) continue;
              toolState.set(payload.tool_use_id, {
                name: payload.name,
                args: toJsonObject(payload.input),
              });
              setStatusText(`tool: ${payload.name}...`);
              const content = makeContent();
              if (content.length > 0) yield { content };
              continue;
            }

            if (event.event === "tool_result") {
              const payload = data as ToolResultPayload;
              if (!payload.tool_use_id || !payload.name) continue;

              const previous = toolState.get(payload.tool_use_id);
              const resultPayload: ReadonlyJSONObject = toJsonObject({
                output: payload.output ?? "",
                duration_ms: payload.duration_ms ?? null,
                bytes: payload.bytes ?? null,
                status_code: payload.status_code ?? null,
                error_type: payload.error_type ?? null,
              });

              toolState.set(payload.tool_use_id, {
                name: payload.name,
                args: previous?.args ?? {},
                result: resultPayload,
                isError: Boolean(payload.is_error),
              });

              const ms =
                typeof payload.duration_ms === "number"
                  ? payload.duration_ms
                  : 0;
              const bytes =
                typeof payload.bytes === "number" ? payload.bytes : 0;
              setStatusText(
                `tool: ${payload.name} ${payload.is_error ? "error" : "ok"} ${ms}ms ${bytes}b`,
              );
              const content = makeContent();
              if (content.length > 0) yield { content };
              continue;
            }

            if (event.event === "delta") {
              const delta = typeof data.delta === "string" ? data.delta : "";
              if (!delta) continue;
              assistantText += delta;
              const content = makeContent();
              if (content.length > 0) yield { content };
              continue;
            }

            if (event.event === "error") {
              const message =
                typeof data.error === "string" ? data.error : "stream error";
              throw new Error(message);
            }

            if (event.event === "done") {
              setStatusText("Done");
              break;
            }
          }
        } catch (e) {
          if (isUnauthorizedError(e)) {
            lockForAuth("Session expired. Please sign in again.");
            setStatusText("Auth required");
          }
          throw e;
        } finally {
          setSending(false);
          void loadSessions();
          void loadHistory(sessionKey);
        }
      },
    }),
    [sessionKey, selectedSessionReadOnly],
  );

  return adapter;
}
