import type { ThreadMessageLike } from "@assistant-ui/react";
import type { HarnessKnowledgeCitation } from "@/services/assistant/harnessService";
import type {
  HarnessAgentRole,
  HarnessAgentState,
  HarnessEvent,
} from "@/services/assistant/harnessContract";

export type AssistantMessageContent = Exclude<ThreadMessageLike["content"], string>;

export interface ProjectionAgentTask {
  readonly runId: string;
  readonly role: HarnessAgentRole;
  readonly objective: string;
  readonly state: HarnessAgentState | "running";
  readonly summary: string | null;
  readonly warnings: readonly string[];
  readonly activity: string | null;
  readonly plan: unknown | null;
}

export function updateAgentTask(
  content: AssistantMessageContent,
  event: HarnessEvent,
): AssistantMessageContent {
  if (
    event.type !== "agent_run_started" &&
    event.type !== "agent_run_finished" &&
    event.type !== "agent_run_output" &&
    event.type !== "agent_run_invalidated"
  )
    return content;
  if (
    (event.type === "agent_run_started" || event.type === "agent_run_finished") &&
    event.payload.role === "manager"
  )
    return content;
  const runId = event.payload.runId;
  const index = content.findIndex(
    (part) =>
      part.type === "data" &&
      part.name === "agent-task" &&
      (part.data as ProjectionAgentTask).runId === runId,
  );
  const previous = index >= 0 ? (content[index] as { data: ProjectionAgentTask }).data : undefined;
  let task: ProjectionAgentTask;
  if (event.type === "agent_run_started") {
    task = {
      runId,
      role: event.payload.role,
      objective: event.payload.objective,
      state: "running",
      summary: null,
      warnings: [],
      activity: null,
      plan: null,
    };
  } else if (!previous) {
    return content;
  } else if (event.type === "agent_run_invalidated") {
    task = { ...previous, state: "stale", activity: null };
  } else if (event.type === "agent_run_finished") {
    task = {
      ...previous,
      state: event.payload.state,
      summary: event.payload.summary,
      warnings: event.payload.warnings,
      activity: null,
    };
  } else {
    const output = event.payload.event;
    if (output.type === "plan_proposed") {
      task = { ...previous, plan: output.payload.plan };
      return content.map((part, i) =>
        i === index ? { type: "data", name: "agent-task", data: task } : part,
      );
    }
    if (
      output.type !== "tool_invocation_started" &&
      output.type !== "tool_invocation_completed" &&
      output.type !== "tool_invocation_failed"
    )
      return content;
    task = {
      ...previous,
      activity: output.type === "tool_invocation_started" ? output.payload.capabilityId : null,
    };
  }
  const part = { type: "data" as const, name: "agent-task", data: task };
  return index < 0 ? [...content, part] : content.map((old, i) => (i === index ? part : old));
}

export function settleAgentTasks(
  content: AssistantMessageContent,
  state: "cancelled" | "interrupted",
): AssistantMessageContent {
  return content.map((part) => {
    if (part.type !== "data" || part.name !== "agent-task") return part;
    const task = part.data as ProjectionAgentTask;
    return task.state === "running" ? { ...part, data: { ...task, state, activity: null } } : part;
  });
}
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
    const index = parts.findIndex(
      (part) => part.type === "tool-call" && part.toolCallId === tool.invocationId,
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
