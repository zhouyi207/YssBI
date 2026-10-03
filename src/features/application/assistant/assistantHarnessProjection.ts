import {
  appendAssistantText,
  finishAssistantText,
  updateAssistantContent,
  updateAgentTask,
  settleAgentTasks,
  type AssistantMessageContent,
  type ProjectionToolCall,
} from "./assistantMessageContent";
import type { ErrorReference } from "@/features/application/errorReference";
import type {
  HarnessEvent,
  HarnessSession,
  HarnessMemoryRecord,
} from "@/services/assistant/harnessService";

type ProjectionStatus = "initializing" | "ready" | "provider-unavailable" | "error";
type ProjectionMessageStatus =
  | Readonly<{ type: "running" }>
  | Readonly<{ type: "complete"; reason: "stop" }>
  | Readonly<{ type: "incomplete"; reason: "cancelled" | "error" }>;

export interface ProjectionMessage {
  readonly id: string;
  readonly role: "user" | "assistant";
  readonly content: AssistantMessageContent;
  readonly createdAt: Date;
  readonly status: ProjectionMessageStatus;
}

export interface AssistantHarnessSnapshot {
  readonly status: ProjectionStatus;
  readonly error: ErrorReference | null;
  readonly providerConfigured: boolean;
  readonly sessionId: string | null;
  readonly conversations: readonly HarnessSession[];
  readonly lastSequence: number;
  readonly messages: readonly ProjectionMessage[];
  readonly isRunning: boolean;
  readonly activity: string | null;
  readonly memoryCount: number;
  readonly memoryRecords: readonly HarnessMemoryRecord[];
}

export const INITIAL_SNAPSHOT: AssistantHarnessSnapshot = Object.freeze({
  status: "initializing",
  error: null,
  providerConfigured: false,
  sessionId: null,
  conversations: Object.freeze([]),
  lastSequence: 0,
  messages: Object.freeze([]),
  isRunning: false,
  activity: null,
  memoryCount: 0,
  memoryRecords: Object.freeze([]),
});

function updateAssistantMessage(
  messages: readonly ProjectionMessage[],
  event: HarnessEvent,
  transform: (content: AssistantMessageContent) => AssistantMessageContent,
  status?: ProjectionMessageStatus,
): readonly ProjectionMessage[] {
  if (!event.turnId) return messages;
  const id = `assistant-${event.turnId}`;
  const existing = messages.find((message) => message.id === id);
  const message: ProjectionMessage = {
    id,
    role: "assistant",
    createdAt: existing?.createdAt ?? new Date(event.occurredAt),
    content: transform(existing?.content ?? []),
    status: status ?? existing?.status ?? { type: "running" },
  };
  return existing
    ? messages.map((previous) => (previous.id === id ? message : previous))
    : [...messages, message];
}

function settleAssistantContent(
  content: AssistantMessageContent,
  state: "cancelled" | "interrupted",
): AssistantMessageContent {
  return settleAgentTasks(content, state).map((part) =>
    part.type === "tool-call" && part.result === undefined
      ? { ...part, result: { status: state }, isError: true }
      : part,
  );
}

function toolFailureState(code: string): ProjectionToolCall["state"] {
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

/** Reduce an accepted ordered event; transport sequence checks remain with the session. */
export function reduceHarnessEvent(
  snapshot: AssistantHarnessSnapshot,
  event: HarnessEvent,
): AssistantHarnessSnapshot {
  const next = { ...snapshot, lastSequence: event.sequence };
  const updateMessage = (
    transform: (content: AssistantMessageContent) => AssistantMessageContent,
    status?: ProjectionMessageStatus,
  ) => {
    next.messages = updateAssistantMessage(next.messages, event, transform, status);
  };

  switch (event.type) {
    case "turn_started":
      if (!event.turnId) break;
      next.messages = [
        ...snapshot.messages,
        {
          id: `user-${event.turnId}`,
          role: "user",
          content: [{ type: "text", text: event.payload.userMessage }],
          createdAt: new Date(event.occurredAt),
          status: { type: "complete", reason: "stop" },
        },
      ];
      next.isRunning = true;
      break;
    case "agent_run_started":
    case "agent_run_finished":
      if (event.payload.role === "manager") break;
      updateMessage((content) => updateAgentTask(content, event));
      break;
    case "agent_run_output":
    case "agent_run_invalidated":
      updateMessage((content) => updateAgentTask(content, event));
      break;
    case "text_delta":
      if (!event.turnId) break;
      updateMessage((content) => appendAssistantText(content, event.payload.delta), {
        type: "running",
      });
      next.isRunning = true;
      break;
    case "turn_completed":
      if (!event.turnId) break;
      updateMessage(
        (content) =>
          finishAssistantText(
            settleAssistantContent(content, "interrupted"),
            event.payload.finalText,
          ),
        { type: "complete", reason: "stop" },
      );
      next.isRunning = false;
      next.activity = null;
      break;
    case "turn_failed":
    case "turn_cancelled":
      if (!event.turnId) break;
      updateMessage(
        (content) =>
          settleAssistantContent(
            content,
            event.type === "turn_cancelled" ? "cancelled" : "interrupted",
          ),
        { type: "incomplete", reason: event.type === "turn_cancelled" ? "cancelled" : "error" },
      );
      next.isRunning = false;
      next.activity = null;
      break;
    case "tool_invocation_started":
    case "tool_invocation_completed":
    case "tool_invocation_failed": {
      if (!event.turnId) break;
      const tool: ProjectionToolCall = {
        invocationId: event.payload.invocationId,
        capabilityId: event.payload.capabilityId,
        state:
          event.type === "tool_invocation_failed"
            ? toolFailureState(event.payload.failureCode)
            : event.type === "tool_invocation_started"
              ? "running"
              : "completed",
      };
      updateMessage((content) => updateAssistantContent(content, [], null, [tool]));
      const content = next.messages.find(
        (message) => message.id === `assistant-${event.turnId}`,
      )?.content;
      const running = content?.find(
        (part) => part.type === "tool-call" && part.result === undefined,
      );
      next.activity = running?.type === "tool-call" ? running.toolName : null;
      break;
    }
    case "workflow_started":
    case "workflow_resumed":
      next.activity = event.payload.runId;
      break;
    case "workflow_completed":
    case "workflow_paused":
    case "workflow_cancelled":
      next.activity = null;
      break;
    case "knowledge_cited":
      updateMessage((content) =>
        updateAssistantContent(content, [event.payload.citation], null, []),
      );
      break;
    case "plan_proposed":
      updateMessage((content) => updateAssistantContent(content, [], event.payload.plan, []));
      break;
    case "memory_recorded":
      next.memoryRecords = [
        ...snapshot.memoryRecords.filter(
          (record) => record.recordId !== event.payload.record.recordId,
        ),
        event.payload.record,
      ];
      next.memoryCount = next.memoryRecords.length;
      break;
    case "memory_deleted":
      next.memoryRecords = snapshot.memoryRecords.filter(
        (record) => record.recordId !== event.payload.recordId,
      );
      next.memoryCount = next.memoryRecords.length;
      break;
  }
  return next;
}
