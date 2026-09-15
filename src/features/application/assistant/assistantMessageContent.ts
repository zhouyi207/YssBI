import type { ThreadMessageLike } from "@assistant-ui/react";
import type { HarnessKnowledgeCitation } from "@/services/assistant/harnessService";

export type AssistantMessageContent = Exclude<ThreadMessageLike["content"], string>;
export interface ProjectionToolCall {
  readonly invocationId: string;
  readonly capabilityId: string;
  readonly state:
    | "running"
    | "completed"
    | "failed"
    | "cancelled"
    | "timed-out"
    | "interrupted"
    | "unknown";
}

export function appendAssistantText(
  content: AssistantMessageContent,
  delta: string,
): AssistantMessageContent {
  if (!delta) return content;
  const last = content[content.length - 1];
  return last?.type === "text"
    ? [...content.slice(0, -1), { ...last, text: last.text + delta }]
    : [...content, { type: "text", text: delta }];
}

export function finishAssistantText(
  content: AssistantMessageContent,
  finalText: string,
): AssistantMessageContent {
  // Completed events also support restored turns with no preceding text events.
  // A streamed transcript is already ordered and must not be replaced or appended twice.
  return content.some((part) => part.type === "text" && part.text.length > 0)
    ? content
    : appendAssistantText(content, finalText);
}

export function updateAssistantContent(
  content: AssistantMessageContent,
  citations: readonly HarnessKnowledgeCitation[],
  plan: unknown | null,
  tools: readonly ProjectionToolCall[],
): AssistantMessageContent {
  const parts = [...content];
  for (const tool of tools) {
    let index = parts.findIndex(
      (part) => part.type === "tool-call" && part.toolCallId === tool.invocationId,
    );
    if (index < 0)
      index = parts.findIndex(
        (part) =>
          part.type === "tool-call" &&
          part.toolCallId?.startsWith("pending-") &&
          part.toolName === tool.capabilityId,
      );
    const part = {
      type: "tool-call" as const,
      toolCallId: tool.invocationId,
      toolName: tool.capabilityId,
      args: {},
      ...(tool.state !== "running"
        ? { result: { status: tool.state }, isError: tool.state !== "completed" }
        : {}),
    };
    if (index < 0) parts.push(part);
    else parts[index] = part;
  }
  for (const citation of citations) {
    if (!parts.some((part) => part.type === "source" && part.id === citation.chunkId)) {
      parts.push({
        type: "source",
        sourceType: "document",
        id: citation.chunkId,
        title: citation.title,
        mediaType: "text/markdown",
      });
    }
  }
  if (plan !== null) {
    const index = parts.findIndex(
      (part) => part.type === "data" && part.name === "statistical-plan",
    );
    const part = { type: "data" as const, name: "statistical-plan", data: plan };
    if (index < 0) parts.push(part);
    else parts[index] = part;
  }
  return parts;
}
