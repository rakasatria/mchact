import type { ToolCallMessagePartProps } from "@assistant-ui/react";
import { asObject, formatUnknown } from "../lib/text-processing";

export function ToolCallCard(props: ToolCallMessagePartProps) {
  const result = asObject(props.result);
  const hasResult = Object.keys(result).length > 0;
  const output = result.output;
  const duration = result.duration_ms;
  const bytes = result.bytes;
  const statusCode = result.status_code;
  const errorType = result.error_type;

  return (
    <div className="tool-card">
      <div className="tool-card-head">
        <span className="tool-card-name">{props.toolName}</span>
        <span
          className={`tool-card-state ${hasResult ? (props.isError ? "error" : "ok") : "running"}`}
        >
          {hasResult ? (props.isError ? "error" : "done") : "running"}
        </span>
      </div>
      {Object.keys(props.args || {}).length > 0 ? (
        <pre className="tool-card-pre">
          {JSON.stringify(props.args, null, 2)}
        </pre>
      ) : null}
      {hasResult ? (
        <div className="tool-card-meta">
          {typeof duration === "number" ? <span>{duration}ms</span> : null}
          {typeof bytes === "number" ? <span>{bytes}b</span> : null}
          {typeof statusCode === "number" ? (
            <span>HTTP {statusCode}</span>
          ) : null}
          {typeof errorType === "string" && errorType ? (
            <span>{errorType}</span>
          ) : null}
        </div>
      ) : null}
      {output !== undefined ? (
        <pre className="tool-card-pre">{formatUnknown(output)}</pre>
      ) : null}
    </div>
  );
}
