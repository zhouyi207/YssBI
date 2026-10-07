import type { ThreadMessageLike } from "@assistant-ui/react";
import type { HarnessKnowledgeCitation } from "@/services/assistant/harnessService";
import type {
  HarnessAgentRole,
  HarnessAgentState,
  HarnessEvent,
  HarnessArtifact,
  HarnessResultReference,
} from "@/services/assistant/harnessContract";

export type AssistantMessageContent = Exclude<ThreadMessageLike["content"], string>;

export interface ProjectionAgentTask {
  readonly runId: string;
  readonly role: HarnessAgentRole;
  readonly objective: string;
  readonly state: HarnessAgentState | "running";
  readonly summary: string | null;
  readonly warnings: readonly string[];
  readonly failureCode: string | null;
  readonly blockedReason: string | null;
  readonly activity: string | null;
  readonly compactionProgress: number | null;
  readonly plan: unknown | null;
  readonly startedAt: number;
  readonly finishedAt: number | null;
  readonly updatedAt: number;
  readonly recoveryAttempt: number;
  readonly tools: AssistantMessageContent;
  readonly artifacts: readonly HarnessArtifact[];
  readonly results: readonly HarnessResultReference[];
}

export function updateAgentTask(
  content: AssistantMessageContent,
  event: HarnessEvent,
): AssistantMessageContent {
  if (
    event.type !== "agent_run_started" &&
    event.type !== "agent_run_resumed" &&
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
  let index = -1;
  for (let i = content.length - 1; i >= 0; i--) {
    const part = content[i];
    if (
      part.type === "data" &&
      part.name === "agent-task" &&
      (part.data as ProjectionAgentTask).runId === runId
    ) {
      index = i;
      break;
    }
  }
  const previous = index >= 0 ? (content[index] as { data: ProjectionAgentTask }).data : undefined;
  let task: ProjectionAgentTask;
  if (event.type === "agent_run_started" || event.type === "agent_run_resumed") {
    // A resumed execution keeps the preceding attempt's duration and outcome in history.
    index = -1;
    task = {
      runId,
      role: event.payload.role,
      objective: event.payload.objective,
      state: "running",
      summary: null,
      warnings: [],
      failureCode: null,
      blockedReason: null,
      activity: null,
      compactionProgress: null,
      plan: null,
      startedAt: event.occurredAt,
      finishedAt: null,
      updatedAt: event.occurredAt,
      recoveryAttempt: 0,
      tools: [],
      artifacts: [],
      results: [],
    };
  } else if (!previous) {
    return content;
  } else if (event.type === "agent_run_invalidated") {
    task = {
      ...previous,
      state: "stale",
      activity: null,
      compactionProgress: null,
      updatedAt: event.occurredAt,
    };
  } else if (event.type === "agent_run_finished") {
    task = {
      ...previous,
      state: event.payload.state,
      artifacts: event.payload.artifacts,
      results: event.payload.results,
      summary: event.payload.summary,
      warnings: [...new Set([...previous.warnings, ...event.payload.warnings])],
      failureCode: event.payload.failureCode,
      blockedReason: event.payload.blockedReason,
      activity: null,
      compactionProgress: null,
      finishedAt: event.occurredAt,
      updatedAt: event.occurredAt,
      tools: settleToolCalls(
        previous.tools,
        event.payload.state === "cancelled" ? "cancelled" : "interrupted",
        event.occurredAt,
      ),
    };
  } else {
    const output = event.payload.event;
    if (output.type === "text_delta" || output.type === "text_retracted") {
      if (
        previous.activity === "writing" &&
        Math.floor(previous.updatedAt / 1000) === Math.floor(event.occurredAt / 1000)
      )
        return content;
      task = { ...previous, activity: "writing", updatedAt: event.occurredAt };
      return content.map((part, i) =>
        i === index ? { type: "data", name: "agent-task", data: task } : part,
      );
    }
    if (output.type === "graph_execution_finished") {
      task = {
        ...previous,
        activity: null,
        compactionProgress: null,
        warnings:
          output.payload.status === "succeeded"
            ? previous.warnings
            : [...previous.warnings, output.payload.failureCode ?? output.payload.status],
        updatedAt: event.occurredAt,
        tools: updateAssistantContent(previous.tools, [], null, [
          {
            invocationId: output.payload.invocationId,
            capabilityId: "execute_graph",
            state: output.payload.status === "succeeded" ? "completed" : output.payload.status,
            executionStatus: output.payload.status,
            failureCode: output.payload.failureCode,
            occurredAt: event.occurredAt,
          },
        ]),
      };
      return content.map((part, i) =>
        i === index ? { type: "data", name: "agent-task", data: task } : part,
      );
    }
    if (output.type === "runtime_status") {
      task = {
        ...previous,
        activity: output.payload.phase,
        compactionProgress: null,
        recoveryAttempt: output.payload.attempt,
        updatedAt: event.occurredAt,
      };
      return content.map((part, i) =>
        i === index ? { type: "data", name: "agent-task", data: task } : part,
      );
    }
    if (output.type === "context_compaction_progress" || output.type === "context_compacted") {
      task =
        output.type === "context_compacted"
          ? { ...previous, activity: null, compactionProgress: null }
          : {
              ...previous,
              activity: "compacting",
              compactionProgress: Math.floor(
                (100 * output.payload.completedBytes) / output.payload.totalBytes,
              ),
              updatedAt: event.occurredAt,
            };
      return content.map((part, i) =>
        i === index ? { type: "data", name: "agent-task", data: task } : part,
      );
    }
    if (output.type === "plan_proposed") {
      task = { ...previous, plan: output.payload.plan, updatedAt: event.occurredAt };
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
      compactionProgress: null,
      updatedAt: event.occurredAt,
      tools: updateAssistantContent(previous.tools, [], null, [
        {
          invocationId: output.payload.invocationId,
          capabilityId: output.payload.capabilityId,
          state:
            output.type === "tool_invocation_failed"
              ? toolFailureState(output.payload.failureCode)
              : output.type === "tool_invocation_started"
                ? "running"
                : "completed",
          failureCode: output.type === "tool_invocation_failed" ? output.payload.failureCode : null,
          occurredAt: event.occurredAt,
        },
      ]),
    };
    const active = task.tools.find(
      (part) => part.type === "tool-call" && part.result === undefined,
    );
    task = { ...task, activity: active?.type === "tool-call" ? (active.toolName ?? null) : null };
  }
  const part = { type: "data" as const, name: "agent-task", data: task };
  return index < 0 ? [...content, part] : content.map((old, i) => (i === index ? part : old));
}

export function settleAgentTasks(
  content: AssistantMessageContent,
  state: "cancelled" | "interrupted",
  finishedAt: number,
): AssistantMessageContent {
  return content.map((part) => {
    if (part.type !== "data" || part.name !== "agent-task") return part;
    const task = part.data as ProjectionAgentTask;
    return task.state === "running"
      ? {
          ...part,
          data: {
            ...task,
            state,
            activity: null,
            compactionProgress: null,
            finishedAt,
            updatedAt: finishedAt,
            tools: settleToolCalls(task.tools, state, finishedAt),
          },
        }
      : part;
  });
}

export function settleToolCalls(
  content: AssistantMessageContent,
  state: "cancelled" | "interrupted",
  finishedAt: number,
): AssistantMessageContent {
  return content.map((part) =>
    part.type === "tool-call" && part.result === undefined
      ? {
          ...part,
          result: { status: state },
          isError: true,
          ...(part.timing ? { timing: { ...part.timing, completedAt: finishedAt } } : {}),
        }
      : part,
  );
}
export interface ProjectionToolCall {
  readonly invocationId: string;
  readonly executionStatus?: "succeeded" | "failed" | "cancelled";
  readonly failureCode?: string | null;
  readonly capabilityId: string;
  readonly occurredAt?: number;
  readonly state:
    | "running"
    | "completed"
    | "failed"
    | "cancelled"
    | "timed-out"
    | "interrupted"
    | "unknown";
}

export function toolFailureState(code: string): ProjectionToolCall["state"] {
  switch (code) {
    case "cancelled":
      return "cancelled";
    case "outcome_unknown":
      return "unknown";
    case "deadline_elapsed":
      return "timed-out";
    default:
      return "failed";
  }
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
    const previous = parts[index];
    const previousTiming = previous?.type === "tool-call" ? previous.timing : undefined;
    const startedAt =
      previousTiming?.startedAt ?? (tool.state === "running" ? tool.occurredAt : undefined);
    const part = {
      type: "tool-call" as const,
      toolCallId: tool.invocationId,
      toolName: tool.capabilityId,
      args: {},
      ...(startedAt !== undefined
        ? {
            timing: {
              startedAt,
              ...(tool.state !== "running"
                ? { completedAt: previousTiming?.completedAt ?? tool.occurredAt }
                : {}),
            },
          }
        : {}),
      ...(tool.state !== "running"
        ? {
            result: {
              status: tool.state,
              executionStatus: tool.executionStatus,
              failureCode: tool.failureCode,
            },
            isError: tool.state !== "completed",
          }
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
        providerMetadata: { yssbi: { ...citation } },
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

export function retractAssistantText(
  content: AssistantMessageContent,
  characters: number,
): AssistantMessageContent {
  const parts = [...content];
  for (let index = parts.length - 1; index >= 0 && characters > 0; index--) {
    const part = parts[index];
    if (part?.type !== "text") continue;
    const text = Array.from(part.text);
    parts[index] = { ...part, text: text.slice(0, Math.max(0, text.length - characters)).join("") };
    characters = Math.max(0, characters - text.length);
  }
  return parts;
}

export function recordAssistantFailure(
  content: AssistantMessageContent,
  code: string,
): AssistantMessageContent {
  const part = { type: "data" as const, name: "assistant-failure", data: { code } };
  const index = content.findIndex(
    (part) => part.type === "data" && part.name === "assistant-failure",
  );
  return index < 0 ? [...content, part] : content.map((old, i) => (i === index ? part : old));
}

export function assistantFailureKey(code: string): string {
  switch (code) {
    case "provider_authentication_failed":
      return "assistant_authentication_failed";
    case "provider_rate_limited":
      return "assistant_rate_limited";
    case "provider_transport_failed":
      return "assistant_provider_connection_failed";
    case "internal_failure":
    case "output_unavailable":
      return "assistant_turn_failed";
    default:
      return `assistant_${code}`;
  }
}
