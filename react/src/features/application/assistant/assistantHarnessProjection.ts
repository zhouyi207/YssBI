import type {
  LanguageModelSelection,
  LanguageModelIdentity,
} from "@/services/assistant/modelContract";
import {
  appendAssistantText,
  retractAssistantText,
  recordAssistantFailure,
  finishAssistantText,
  updateAssistantContent,
  updateAgentTask,
  settleAgentTasks,
  settleToolCalls,
  toolFailureState,
  type AssistantMessageContent,
  type ProjectionToolCall,
} from "./assistantMessageContent";
import type { ErrorReference } from "@/features/application/errorReference";
import type { PendingAssistantMessage, QueuedAssistantMessage } from "./assistantDrafts";
import type { HarnessEvent } from "@/services/assistant/harnessService";
import type { HarnessResourceReference } from "@/services/assistant/harnessContract";
import type { ResourceRef } from "@/shared/types/domain/resource";
import {
  DEFAULT_TURN_OPTIONS,
  type HarnessTurnOptions,
} from "@/services/assistant/harnessContract";
import { EMPTY_USAGE, accumulateUsage, type AssistantUsage } from "./assistantUsage";

type ProjectionStatus = "initializing" | "ready" | "provider-unavailable" | "error";
type ProjectionMessageStatus =
  | Readonly<{ type: "running" }>
  | Readonly<{ type: "complete"; reason: "stop" }>
  | Readonly<{ type: "incomplete"; reason: "cancelled" | "error" }>;

export interface ProjectionMessage {
  readonly options?: HarnessTurnOptions;
  readonly resources: readonly HarnessResourceReference[];
  readonly model: LanguageModelIdentity | null;
  readonly id: string;
  readonly role: "user" | "assistant";
  readonly content: AssistantMessageContent;
  readonly createdAt: Date;
  readonly status: ProjectionMessageStatus;
  readonly finishedAt: number | null;
  readonly updatedAt: number;
}

export interface AssistantHarnessSnapshot {
  readonly turnOptions: HarnessTurnOptions;
  readonly usage: AssistantUsage;
  readonly draftResources: readonly ResourceRef[];
  readonly selectedModel: LanguageModelSelection | null;
  readonly selectingModel: boolean;
  readonly status: ProjectionStatus;
  readonly error: ErrorReference | null;
  readonly providerConfigured: boolean;
  readonly sessionId: string | null;
  readonly lastSequence: number;
  readonly messages: readonly ProjectionMessage[];
  readonly isRunning: boolean;
  readonly isSubmitting: boolean;
  readonly isStopping: boolean;
  readonly activity: string | null;
  readonly recoveryAttempt: number;
  readonly compactionProgress: number | null;
  readonly unsentMessage: PendingAssistantMessage | null;
  readonly queuedMessages: readonly QueuedAssistantMessage[];
  readonly queuePaused: boolean;
}

export const INITIAL_SNAPSHOT: AssistantHarnessSnapshot = Object.freeze({
  turnOptions: DEFAULT_TURN_OPTIONS,
  usage: EMPTY_USAGE,
  draftResources: Object.freeze([]),
  selectedModel: null,
  selectingModel: false,
  status: "initializing",
  error: null,
  providerConfigured: false,
  sessionId: null,
  lastSequence: 0,
  messages: Object.freeze([]),
  isRunning: false,
  isSubmitting: false,
  isStopping: false,
  activity: null,
  recoveryAttempt: 0,
  compactionProgress: null,
  unsentMessage: null,
  queuedMessages: Object.freeze([]),
  queuePaused: true,
});

function updateAssistantMessage(
  messages: readonly ProjectionMessage[],
  event: HarnessEvent,
  transform: (content: AssistantMessageContent) => AssistantMessageContent,
  status?: ProjectionMessageStatus,
): readonly ProjectionMessage[] {
  if (!event.turnId) return messages;
  const id = `assistant-${event.turnId}`;
  const lastIndex = messages.length - 1;
  const index =
    messages[lastIndex]?.id === id ? lastIndex : messages.findIndex((message) => message.id === id);
  const existing = messages[index];
  const terminal = ["turn_completed", "turn_failed", "turn_cancelled"].includes(event.type);
  const message: ProjectionMessage = {
    options: event.type === "turn_configured" ? event.payload.options : existing?.options,
    resources: existing?.resources ?? [],
    model: existing?.model ?? (event.type === "turn_started" ? event.payload.model : null),
    id,
    role: "assistant",
    createdAt: existing?.createdAt ?? new Date(event.occurredAt),
    content: transform(existing?.content ?? []),
    status: status ?? existing?.status ?? { type: "running" },
    finishedAt: terminal ? event.occurredAt : (existing?.finishedAt ?? null),
    updatedAt: event.occurredAt,
  };
  const next = [...messages];
  if (index < 0) next.push(message);
  else next[index] = message;
  return next;
}

function settleAssistantContent(
  content: AssistantMessageContent,
  state: "cancelled" | "interrupted",
  finishedAt: number,
): AssistantMessageContent {
  return settleToolCalls(settleAgentTasks(content, state, finishedAt), state, finishedAt);
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
    case "turn_configured":
      updateMessage((content) => content);
      break;
    case "usage_reported":
      next.usage = accumulateUsage(next.usage, event.payload);
      break;
    case "reasoning_delta":
      updateMessage((content) => {
        const last = content[content.length - 1];
        return last?.type === "reasoning"
          ? [...content.slice(0, -1), { ...last, text: last.text + event.payload.delta }]
          : [...content, { type: "reasoning", text: event.payload.delta }];
      });
      break;
    case "turn_started":
      if (!event.turnId) break;
      next.activity = null;
      next.usage = EMPTY_USAGE;
      next.isStopping = false;
      next.recoveryAttempt = 0;
      next.compactionProgress = null;
      next.messages = [
        ...snapshot.messages,
        {
          model: event.payload.model,
          id: `user-${event.turnId}`,
          resources: event.payload.resources,
          role: "user",
          content: [{ type: "text", text: event.payload.userMessage }],
          createdAt: new Date(event.occurredAt),
          status: { type: "complete", reason: "stop" },
          finishedAt: event.occurredAt,
          updatedAt: event.occurredAt,
        },
      ];
      updateMessage((content) => content, { type: "running" });
      next.isRunning = true;
      break;
    case "agent_run_finished":
      if (event.payload.role === "manager") {
        if (event.payload.artifacts.length > 0 || event.payload.results.length > 0)
          updateMessage((content) => [
            ...content,
            {
              type: "data",
              name: "artifacts",
              data: { artifacts: event.payload.artifacts, results: event.payload.results },
            },
          ]);
        if (event.payload.state === "failed" || event.payload.state === "blocked") {
          updateMessage(
            (content) =>
              recordAssistantFailure(
                content,
                event.payload.failureCode ?? "report_delivery_incomplete",
              ),
            { type: "incomplete", reason: "error" },
          );
        }
        break;
      }
      updateMessage((content) => updateAgentTask(content, event));
      break;
    case "agent_run_started":
    case "agent_run_resumed":
      if (event.payload.role === "manager") break;
      updateMessage((content) => updateAgentTask(content, event));
      break;
    case "agent_run_output":
      if (event.payload.event.type === "usage_reported") {
        next.usage = accumulateUsage(next.usage, event.payload.event.payload, true);
        break;
      }
      if (event.payload.event.type === "knowledge_cited") {
        const { citation } = event.payload.event.payload;
        updateMessage((content) => updateAssistantContent(content, [citation], null, []));
        break;
      }
      updateMessage((content) => updateAgentTask(content, event));
      break;
    case "agent_run_invalidated":
      updateMessage((content) => updateAgentTask(content, event));
      break;
    case "runtime_status":
      next.activity = event.payload.phase;
      next.recoveryAttempt = event.payload.attempt;
      if (next.activity !== "compacting") next.compactionProgress = null;
      break;
    case "context_compaction_progress":
      next.activity = "compacting";
      next.compactionProgress = Math.floor(
        (100 * event.payload.completedBytes) / event.payload.totalBytes,
      );
      break;
    case "context_compacted":
      next.usage = { ...next.usage, latest: null };
      next.activity = null;
      next.compactionProgress = null;
      break;
    case "delivery_blocked":
      updateMessage((content) => recordAssistantFailure(content, "report_delivery_incomplete"), {
        type: "incomplete",
        reason: "error",
      });
      break;
    case "text_retracted":
      updateMessage((content) => retractAssistantText(content, event.payload.characters));
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
            settleAssistantContent(content, "interrupted", event.occurredAt),
            event.payload.finalText,
          ),
        next.messages.find((message) => message.id === `assistant-${event.turnId}`)?.status.type ===
          "incomplete"
          ? { type: "incomplete", reason: "error" }
          : { type: "complete", reason: "stop" },
      );
      next.isRunning = false;
      next.isStopping = false;
      next.activity = null;
      next.compactionProgress = null;
      break;
    case "turn_failed":
    case "turn_cancelled":
      if (!event.turnId) break;
      next.queuePaused = true;
      updateMessage(
        (content) =>
          settleAssistantContent(
            content,
            event.type === "turn_cancelled" ? "cancelled" : "interrupted",
            event.occurredAt,
          ),
        { type: "incomplete", reason: event.type === "turn_cancelled" ? "cancelled" : "error" },
      );
      next.isRunning = false;
      next.isStopping = false;
      next.activity = null;
      next.compactionProgress = null;
      break;
    case "graph_execution_finished":
      updateMessage((content) =>
        updateAssistantContent(content, [], null, [
          {
            invocationId: event.payload.invocationId,
            capabilityId: "execute_graph",
            state: event.payload.status === "succeeded" ? "completed" : event.payload.status,
            executionStatus: event.payload.status,
            failureCode: event.payload.failureCode,
            occurredAt: event.occurredAt,
          },
        ]),
      );
      break;
    case "tool_invocation_started":
    case "tool_invocation_completed":
    case "tool_invocation_failed": {
      if (!event.turnId) break;
      const tool: ProjectionToolCall = {
        invocationId: event.payload.invocationId,
        capabilityId: event.payload.capabilityId,
        occurredAt: event.occurredAt,
        failureCode: event.type === "tool_invocation_failed" ? event.payload.failureCode : null,
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
  }
  return next;
}
