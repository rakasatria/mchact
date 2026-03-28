import {
  AssistantRuntimeProvider,
  useLocalRuntime,
  type ChatModelAdapter,
  type ThreadMessageLike,
} from "@assistant-ui/react";
import { Thread, makeMarkdownText } from "@assistant-ui/react-ui";
import remarkGfm from "remark-gfm";
import remarkBreaks from "remark-breaks";
import { extractThinkSegments } from "../lib/text-processing";
import { ToolCallCard } from "./tool-call-card";
import { CustomAssistantMessage, CustomUserMessage } from "./message-components";

export type ThreadPaneProps = {
  adapter: ChatModelAdapter;
  initialMessages: ThreadMessageLike[];
  runtimeKey: string;
};

export function ThreadPane({ adapter, initialMessages, runtimeKey }: ThreadPaneProps) {
  const MarkdownText = makeMarkdownText({
    preprocess: (text) => extractThinkSegments(text).visibleText,
    remarkPlugins: [remarkGfm, remarkBreaks],
  });
  const runtime = useLocalRuntime(adapter, {
    initialMessages,
    maxSteps: 100,
  });

  return (
    <AssistantRuntimeProvider key={runtimeKey} runtime={runtime}>
      <div className="aui-root h-full min-h-0">
        <Thread
          assistantMessage={{
            allowCopy: true,
            allowReload: false,
            allowSpeak: false,
            allowFeedbackNegative: false,
            allowFeedbackPositive: false,
            components: {
              Text: MarkdownText,
              ToolFallback: ToolCallCard,
            },
          }}
          userMessage={{ allowEdit: false }}
          composer={{ allowAttachments: false }}
          components={{
            AssistantMessage: CustomAssistantMessage,
            UserMessage: CustomUserMessage,
          }}
          strings={{
            composer: {
              input: { placeholder: "Message mchact..." },
            },
          }}
          assistantAvatar={{ fallback: "M" }}
        />
      </div>
    </AssistantRuntimeProvider>
  );
}
