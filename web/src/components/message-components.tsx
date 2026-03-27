import { useMessage, MessagePrimitive } from "@assistant-ui/react";
import {
  AssistantMessage,
  BranchPicker,
  AssistantActionBar,
  UserMessage,
  UserActionBar,
} from "@assistant-ui/react-ui";
import { collectThinkText, extractThinkSegments } from "../lib/text-processing";
import type { MediaAttachment } from "../lib/types";
import { ImageViewer } from "./media/image-viewer";
import { AudioPlayer } from "./media/audio-player";
import { VideoPlayer } from "./media/video-player";
import { FilePreview } from "./media/file-preview";

export function MessageTimestamp({ align }: { align: "left" | "right" }) {
  const createdAt = useMessage((m) => m.createdAt);
  const formatted = createdAt
    ? createdAt.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
    : "";
  return (
    <div
      className={
        align === "right" ? "mc-msg-time mc-msg-time-right" : "mc-msg-time"
      }
    >
      {formatted}
    </div>
  );
}

export function CustomAssistantMessage() {
  const thinkText = useMessage((m) =>
    collectThinkText(
      m.content as readonly { type: string; text?: string }[] | undefined,
    ),
  );

  const hasRenderableContent = useMessage((m) =>
    Array.isArray(m.content)
      ? m.content.some((part) => {
          if (part.type === "text")
            return Boolean(
              extractThinkSegments(part.text ?? "").visibleText.trim(),
            );
          return part.type === "tool-call";
        })
      : false,
  );

  const attachments = useMessage((m) => {
    const meta = (m as { metadata?: Record<string, unknown> }).metadata;
    return (meta?.attachments as MediaAttachment[] | undefined) ?? [];
  });

  return (
    <AssistantMessage.Root>
      <AssistantMessage.Avatar />
      {hasRenderableContent ? (
        <AssistantMessage.Content />
      ) : (
        <div className="mc-assistant-placeholder" aria-live="polite">
          <span className="mc-assistant-placeholder-dot" />
          <span className="mc-assistant-placeholder-dot" />
          <span className="mc-assistant-placeholder-dot" />
          <span className="mc-assistant-placeholder-text">Thinking</span>
        </div>
      )}
      {attachments.length > 0 && (
        <div className="mc-attachments">
          {attachments.map((att, i) => (
            <div key={i}>
              {att.type === "image" && <ImageViewer src={att.url} />}
              {att.type === "audio" && (
                <AudioPlayer src={att.url} name={att.name} />
              )}
              {att.type === "video" && <VideoPlayer src={att.url} />}
              {att.type === "file" && (
                <FilePreview
                  url={att.url}
                  name={att.name || "file"}
                  size={att.size}
                />
              )}
            </div>
          ))}
        </div>
      )}
      {thinkText.trim() ? (
        <details className="mc-think-details" open>
          <summary>
            <span className="mc-think-summary-icon" aria-hidden="true" />
            <span>Thinking & Processing ...</span>
          </summary>
          <pre className="mc-think-content">{thinkText}</pre>
        </details>
      ) : null}
      <BranchPicker />
      <AssistantActionBar />
      <MessageTimestamp align="left" />
    </AssistantMessage.Root>
  );
}

export function CustomUserMessage() {
  return (
    <UserMessage.Root>
      <UserMessage.Attachments />
      <MessagePrimitive.If hasContent>
        <UserActionBar />
        <div className="mc-user-content-wrap">
          <UserMessage.Content />
          <MessageTimestamp align="right" />
        </div>
      </MessagePrimitive.If>
      <BranchPicker />
    </UserMessage.Root>
  );
}
