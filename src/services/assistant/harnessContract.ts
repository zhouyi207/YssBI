import {
  languageModelSelectionSchema,
  languageModelIdentitySchema,
  reasoningEffortSchema,
  type LanguageModelSelection,
  type LanguageModelIdentity,
} from "./modelContract";
import { z } from "zod";
import { isResultReference } from "@/shared/types/domain/result";
import { RESOURCE_KINDS } from "@/shared/types/domain/resource";

export const harnessTurnOptionsSchema = z.strictObject({
  mode: z.enum(["ask", "write"]),
  reasoningEffort: reasoningEffortSchema.nullable(),
});
export type HarnessTurnOptions = z.infer<typeof harnessTurnOptionsSchema>;
export const DEFAULT_TURN_OPTIONS: HarnessTurnOptions = Object.freeze({
  mode: "write",
  reasoningEffort: null,
});
const tokenCount = z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER).nullable();
export const modelTokenUsageSchema = z.strictObject({
  inputTokens: tokenCount,
  outputTokens: tokenCount,
  cachedInputTokens: tokenCount,
  cacheCreationInputTokens: tokenCount,
  reasoningTokens: tokenCount,
});
export type ModelTokenUsage = z.infer<typeof modelTokenUsageSchema>;
const usageReportSchema = z.strictObject({
  usage: modelTokenUsageSchema,
  contextWindow: z.number().int().positive().nullable(),
  purpose: z.enum(["response", "compaction"]),
});
export type HarnessUsageReport = z.infer<typeof usageReportSchema>;

export const harnessResourceRefSchema = z.strictObject({
  kind: z.enum(RESOURCE_KINDS),
  id: z.string().min(1),
});
export const harnessResourceReferenceSchema = z.strictObject({
  resource: harnessResourceRefSchema,
  name: z.string().min(1),
});
export type HarnessResourceReference = z.infer<typeof harnessResourceReferenceSchema>;
const resourceReferencesSchema = z.array(harnessResourceReferenceSchema);

export const harnessArtifactSchema = z.object({
  resource: z.object({
    kind: z.enum(RESOURCE_KINDS),
    id: z.string().min(1),
  }),
  revision: z.number().int().nonnegative(),
  revisionKind: z.enum(["resource", "function_signature"]),
  deleted: z.boolean(),
});
export type HarnessArtifact = z.infer<typeof harnessArtifactSchema>;
const artifactsSchema = z.array(harnessArtifactSchema);
const resultSchema = z
  .object({ executionSessionId: z.string(), resultId: z.string(), output: z.string() })
  .refine(isResultReference);
const resultsSchema = z.array(resultSchema);
export type HarnessResultReference = z.infer<typeof resultSchema>;
const toolInspectionSchema = z.object({
  target: z.string().nullable(),
  parameters: z.record(z.string(), z.unknown()),
  artifacts: artifactsSchema,
  results: resultsSchema,
  startedAt: z.number().int().nonnegative(),
  finishedAt: z.number().int().nonnegative().nullable(),
  failure: z.object({ code: z.string(), details: z.record(z.string(), z.string()) }).nullable(),
});
export type HarnessToolInspection = z.infer<typeof toolInspectionSchema>;
export function parseHarnessToolInspection(value: unknown): HarnessToolInspection {
  const parsed = toolInspectionSchema.safeParse(value);
  if (!parsed.success) throw new InvalidHarnessPayloadError("HarnessToolInspection");
  return parsed.data;
}

const HARNESS_CAPABILITY_IDS = [
  "inspect_resource",
  "create_resource",
  "import_database",
  "rename_resource",
  "duplicate_resource",
  "delete_resource",
  "save_resource",
  "undo_resource",
  "redo_resource",
  "insert_rows",
  "update_cells",
  "delete_rows",
  "create_columns",
  "rename_columns",
  "delete_columns",
  "cast_columns",
  "set_column_semantics",
  "edit_resource",
  "export_database",
  "inspect_ui_intent",
  "request_ui_intent",
  "inspect_graph",
  "browse_nodes",
  "inspect_node_type",
  "find_nodes",
  "find_constants",
  "inspect_constants",
  "create_constants",
  "update_constants",
  "delete_constants",
  "inspect_nodes",
  "find_connections",
  "search_knowledge",
  "read_knowledge",
  "inspect_database",
  "inspect_database_schema",
  "profile_database",
  "read_database_rows",
  "inspect_mind",
  "inspect_chart",
  "update_chart",
  "inspect_document",
  "read_document",
  "search_document",
  "replace_document_text",
  "append_document",
  "write_document",
  "find_topics",
  "inspect_topics",
  "create_topics",
  "update_topics",
  "move_topics",
  "delete_topics",
  "duplicate_topics",
  "inspect_dataset_schema",
  "inspect_dataset_profile",
  "inspect_result",
  "read_result_table",
  "list_resources",
  "apply_graph_edit",
  "create_nodes",
  "update_nodes",
  "delete_nodes",
  "duplicate_nodes",
  "move_nodes",
  "create_connections",
  "update_connections",
  "delete_connections",
  "validate_graph",
  "execute_graph",
  "save_graph",
  "list_graph_results",
] as const;
export type HarnessCapabilityId = (typeof HARNESS_CAPABILITY_IDS)[number];
const HARNESS_CONTROL_TOOLS = [
  "delegate_task",
  "followup_task",
  "propose_statistical_plan",
] as const;
export type HarnessToolId = HarnessCapabilityId | (typeof HARNESS_CONTROL_TOOLS)[number];

const AGENT_ROLES = ["manager", "data", "stats", "plot", "report", "review"] as const;
export type HarnessAgentRole = (typeof AGENT_ROLES)[number];
const AGENT_STATES = [
  "completed",
  "stale",
  "blocked",
  "failed",
  "cancelled",
  "interrupted",
] as const;
export type HarnessAgentState = (typeof AGENT_STATES)[number];

export interface HarnessKnowledgeCitation {
  readonly sourceId: string;
  readonly documentId: string;
  readonly chunkId: string;
  readonly title: string;
  readonly version: string;
  readonly sourceHash: string;
}

export type HarnessEvent = Readonly<{
  sequence: number;
  sessionId: string;
  turnId: string | null;
  occurredAt: number;
}> &
  (
    | Readonly<{ type: "turn_configured"; payload: { options: HarnessTurnOptions } }>
    | Readonly<{ type: "reasoning_delta"; payload: { delta: string } }>
    | Readonly<{ type: "usage_reported"; payload: HarnessUsageReport }>
    | Readonly<{
        type: "agent_run_resumed";
        payload: { runId: string; role: HarnessAgentRole; objective: string };
      }>
    | Readonly<{
        type: "graph_execution_finished";
        payload: {
          invocationId: string;
          status: "succeeded" | "failed" | "cancelled";
          failureCode: string | null;
        };
      }>
    | Readonly<{ type: "text_retracted"; payload: { characters: number } }>
    | Readonly<{ type: "context_compacted" }>
    | Readonly<{
        type: "context_compaction_progress";
        payload: { completedBytes: number; totalBytes: number };
      }>
    | Readonly<{
        type: "runtime_status";
        payload: { phase: "compacting" | "reconnecting" | "checking_delivery"; attempt: number };
      }>
    | Readonly<{ type: "delivery_blocked"; payload: { reason: string } }>
    | Readonly<{ type: "agent_run_invalidated"; payload: { runId: string } }>
    | Readonly<{
        type: "agent_run_started";
        payload: {
          runId: string;
          parentRunId: string | null;
          role: HarnessAgentRole;
          objective: string;
        };
      }>
    | Readonly<{ type: "agent_run_output"; payload: { runId: string; event: HarnessEvent } }>
    | Readonly<{
        type: "agent_run_finished";
        payload: {
          runId: string;
          role: HarnessAgentRole;
          state: HarnessAgentState;
          failureCode: string | null;
          summary: string | null;
          blockedReason: string | null;
          warnings: readonly string[];
          evidenceCount: number;
          artifacts: readonly HarnessArtifact[];
          results: readonly HarnessResultReference[];
        };
      }>
    | Readonly<{ type: "session_created" | "turn_failed" | "turn_cancelled" }>
    | Readonly<{
        type: "turn_started";
        payload: {
          userMessage: string;
          model: LanguageModelIdentity;
          resources: readonly HarnessResourceReference[];
        };
      }>
    | Readonly<{ type: "text_delta"; payload: { delta: string } }>
    | Readonly<{ type: "plan_proposed"; payload: { plan: unknown } }>
    | Readonly<{
        type: "tool_invocation_started" | "tool_invocation_completed";
        payload: { invocationId: string; capabilityId: HarnessToolId };
      }>
    | Readonly<{
        type: "tool_invocation_failed";
        payload: { invocationId: string; capabilityId: HarnessToolId; failureCode: string };
      }>
    | Readonly<{ type: "turn_completed"; payload: { finalText: string } }>
    | Readonly<{ type: "knowledge_cited"; payload: { citation: HarnessKnowledgeCitation } }>
    | Readonly<{
        type:
          | "workflow_planned"
          | "workflow_started"
          | "workflow_completed"
          | "workflow_paused"
          | "workflow_resumed"
          | "workflow_cancelled";
        payload: { runId: string };
      }>
    | Readonly<{
        type: "workflow_step_started" | "workflow_step_completed";
        payload: { runId: string; stepId: string };
      }>
    | Readonly<{
        type: "workflow_step_failed";
        payload: { runId: string; stepId: string; retriable: boolean };
      }>
  );

export interface HarnessSession {
  readonly sessionId: string;
  readonly projectInstanceId: string;
  readonly projectSessionId: string;
  readonly title: string;
  readonly lastOpenedAt: number;
  readonly model: LanguageModelSelection | null;
}

export interface HarnessTurnResult {
  readonly finalText: string;
}

export interface HarnessSubscriptionSnapshot {
  readonly subscriptionId: string;
}

export class InvalidHarnessPayloadError extends Error {
  constructor(readonly payloadName: string) {
    super(`Invalid ${payloadName} payload`);
    this.name = "InvalidHarnessPayloadError";
  }
}

const TOOL_IDS = new Set<HarnessToolId>([...HARNESS_CAPABILITY_IDS, ...HARNESS_CONTROL_TOOLS]);

function record(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function stringField(value: Record<string, unknown>, key: string): string | null {
  const field = value[key];
  return typeof field === "string" && field.length > 0 ? field : null;
}

function integerField(value: Record<string, unknown>, key: string): number | null {
  const field = value[key];
  return typeof field === "number" && Number.isSafeInteger(field) && field >= 0 ? field : null;
}

function payload(value: Record<string, unknown>): Record<string, unknown> | null {
  return record(value.payload);
}

function toolIdentity(value: unknown): HarnessToolId | null {
  return typeof value === "string" && TOOL_IDS.has(value as HarnessToolId)
    ? (value as HarnessToolId)
    : null;
}

function citation(value: unknown): HarnessKnowledgeCitation | null {
  const source = record(value);
  if (!source) return null;
  const sourceId = stringField(source, "sourceId");
  const documentId = stringField(source, "documentId");
  const chunkId = stringField(source, "chunkId");
  const title = stringField(source, "title");
  const version = stringField(source, "version");
  const sourceHash = stringField(source, "sourceHash");
  return sourceId && documentId && chunkId && title && version && sourceHash
    ? { sourceId, documentId, chunkId, title, version, sourceHash }
    : null;
}

export function parseHarnessSession(value: unknown): HarnessSession {
  const source = record(value);
  const sessionId = source && stringField(source, "sessionId");
  const projectInstanceValue = source && stringField(source, "projectInstanceId");
  const projectSessionId = source && stringField(source, "projectSessionId");
  const model = languageModelSelectionSchema.nullable().safeParse(source?.model);
  if (
    !sessionId ||
    !projectInstanceValue ||
    !projectSessionId ||
    typeof source?.title !== "string" ||
    typeof source.lastOpenedAt !== "number" ||
    !Number.isSafeInteger(source.lastOpenedAt) ||
    source.lastOpenedAt < 0 ||
    !model.success
  ) {
    throw new InvalidHarnessPayloadError("HarnessSession");
  }
  return {
    sessionId,
    projectInstanceId: projectInstanceValue,
    projectSessionId,
    title: source.title,
    lastOpenedAt: source.lastOpenedAt,
    model: model.data,
  };
}

export function parseHarnessTurnResult(value: unknown): HarnessTurnResult {
  const source = record(value);
  const finalText = source?.finalText;
  if (typeof finalText !== "string") throw new InvalidHarnessPayloadError("HarnessTurnResult");
  return { finalText };
}

export function parseHarnessSubscription(value: unknown): HarnessSubscriptionSnapshot {
  const source = record(value);
  const subscriptionId = source && stringField(source, "subscriptionId");
  if (!subscriptionId) throw new InvalidHarnessPayloadError("HarnessSubscription");
  return { subscriptionId };
}

export function parseHarnessEvent(value: unknown): HarnessEvent {
  const source = record(value);
  const sequence = source && integerField(source, "sequence");
  const sessionId = source && stringField(source, "sessionId");
  const occurredAt = source && integerField(source, "occurredAt");
  const turnIdValue = source?.turnId;
  const type = source?.type;
  if (
    sequence === null ||
    !sessionId ||
    occurredAt === null ||
    (turnIdValue !== null && typeof turnIdValue !== "string") ||
    typeof type !== "string"
  ) {
    throw new InvalidHarnessPayloadError("HarnessEvent");
  }
  const base = { sequence, sessionId, turnId: turnIdValue as string | null, occurredAt };
  if (["session_created", "turn_failed", "turn_cancelled", "context_compacted"].includes(type)) {
    return { ...base, type } as HarnessEvent;
  }
  const eventPayload = payload(source);
  if (!eventPayload) throw new InvalidHarnessPayloadError("HarnessEvent");
  if (type === "turn_configured") {
    const parsed = harnessTurnOptionsSchema.safeParse(eventPayload.options);
    if (parsed.success) return { ...base, type, payload: { options: parsed.data } };
  }
  if (type === "usage_reported") {
    const parsed = usageReportSchema.safeParse(eventPayload);
    if (parsed.success) return { ...base, type, payload: parsed.data };
  }
  if (type === "reasoning_delta" && typeof eventPayload.delta === "string") {
    return { ...base, type, payload: { delta: eventPayload.delta } };
  }
  if (type === "graph_execution_finished") {
    const invocationId = stringField(eventPayload, "invocationId");
    const { status, failureCode } = eventPayload;
    if (
      invocationId &&
      (status === "succeeded" || status === "failed" || status === "cancelled") &&
      (failureCode === null || typeof failureCode === "string")
    ) {
      return { ...base, type, payload: { invocationId, status, failureCode } };
    }
  }
  if (type === "text_retracted") {
    const characters = integerField(eventPayload, "characters");
    if (characters !== null) return { ...base, type, payload: { characters } };
  } else if (type === "context_compaction_progress") {
    const completedBytes = integerField(eventPayload, "completedBytes");
    const totalBytes = integerField(eventPayload, "totalBytes");
    if (
      completedBytes !== null &&
      totalBytes !== null &&
      totalBytes > 0 &&
      completedBytes <= totalBytes
    ) {
      return { ...base, type, payload: { completedBytes, totalBytes } };
    }
  } else if (type === "runtime_status") {
    const { phase } = eventPayload;
    const attempt = integerField(eventPayload, "attempt");
    if (
      (phase === "compacting" || phase === "reconnecting" || phase === "checking_delivery") &&
      attempt !== null
    ) {
      return { ...base, type, payload: { phase, attempt } };
    }
  } else if (type === "delivery_blocked") {
    const reason = stringField(eventPayload, "reason");
    if (reason) return { ...base, type, payload: { reason } };
  } else if (type === "agent_run_resumed") {
    const runId = stringField(eventPayload, "runId");
    const { role, objective } = eventPayload;
    if (runId && AGENT_ROLES.includes(role as HarnessAgentRole) && typeof objective === "string") {
      return { ...base, type, payload: { runId, role: role as HarnessAgentRole, objective } };
    }
  }
  if (type === "agent_run_invalidated") {
    const runId = stringField(eventPayload, "runId");
    if (runId) return { ...base, type, payload: { runId } };
  }
  if (type === "agent_run_started") {
    const runId = stringField(eventPayload, "runId");
    const { parentRunId, role, objective } = eventPayload;
    if (
      runId &&
      (parentRunId === null || (typeof parentRunId === "string" && parentRunId.length > 0)) &&
      AGENT_ROLES.includes(role as HarnessAgentRole) &&
      typeof objective === "string"
    ) {
      return {
        ...base,
        type,
        payload: { runId, parentRunId, role: role as HarnessAgentRole, objective },
      };
    }
  } else if (type === "agent_run_output") {
    const runId = stringField(eventPayload, "runId");
    const event = record(eventPayload.event);
    if (
      runId &&
      event &&
      [
        "usage_reported",
        "reasoning_delta",
        "graph_execution_finished",
        "text_delta",
        "text_retracted",
        "context_compacted",
        "context_compaction_progress",
        "runtime_status",
        "delivery_blocked",
        "plan_proposed",
        "knowledge_cited",
        "tool_invocation_started",
        "tool_invocation_completed",
        "tool_invocation_failed",
      ].includes(event.type as string)
    ) {
      return { ...base, type, payload: { runId, event: parseHarnessEvent({ ...base, ...event }) } };
    }
  } else if (type === "agent_run_finished") {
    const runId = stringField(eventPayload, "runId");
    const evidenceCount = integerField(eventPayload, "evidenceCount");
    const { role, state, summary, blockedReason, warnings, failureCode } = eventPayload;
    const artifacts = artifactsSchema.safeParse(eventPayload.artifacts);
    const results = resultsSchema.safeParse(eventPayload.results);
    if (
      runId &&
      evidenceCount !== null &&
      artifacts.success &&
      results.success &&
      AGENT_ROLES.includes(role as HarnessAgentRole) &&
      AGENT_STATES.includes(state as HarnessAgentState) &&
      (failureCode === null || typeof failureCode === "string") &&
      (summary === null || typeof summary === "string") &&
      (blockedReason === null || typeof blockedReason === "string") &&
      Array.isArray(warnings) &&
      warnings.every((warning) => typeof warning === "string")
    ) {
      return {
        ...base,
        type,
        payload: {
          runId,
          role: role as HarnessAgentRole,
          state: state as HarnessAgentState,
          failureCode,
          summary,
          blockedReason,
          warnings,
          evidenceCount,
          artifacts: artifacts.data,
          results: results.data,
        },
      };
    }
  }
  if (type === "turn_started") {
    const userMessage = stringField(eventPayload, "userMessage");
    const model = languageModelIdentitySchema.safeParse(eventPayload.model);
    const resources = resourceReferencesSchema.safeParse(eventPayload.resources);
    if (userMessage && model.success && resources.success)
      return {
        ...base,
        type,
        payload: { userMessage, model: model.data, resources: resources.data },
      };
  } else if (type === "text_delta") {
    const delta = eventPayload.delta;
    if (typeof delta === "string") return { ...base, type, payload: { delta } };
  } else if (type === "plan_proposed" && record(eventPayload.plan)) {
    return { ...base, type, payload: { plan: eventPayload.plan } };
  } else if (type === "tool_invocation_started" || type === "tool_invocation_completed") {
    const invocationId = stringField(eventPayload, "invocationId");
    const capabilityId = toolIdentity(eventPayload.capabilityId);
    if (invocationId && capabilityId) {
      return { ...base, type, payload: { invocationId, capabilityId } };
    }
  } else if (type === "tool_invocation_failed") {
    const invocationId = stringField(eventPayload, "invocationId");
    const capabilityId = toolIdentity(eventPayload.capabilityId);
    const failureCode = stringField(eventPayload, "failureCode");
    if (invocationId && capabilityId && failureCode) {
      return { ...base, type, payload: { invocationId, capabilityId, failureCode } };
    }
  } else if (type === "turn_completed") {
    const finalText = eventPayload.finalText;
    if (typeof finalText === "string") return { ...base, type, payload: { finalText } };
  } else if (type === "knowledge_cited") {
    const parsedCitation = citation(eventPayload.citation);
    if (parsedCitation) return { ...base, type, payload: { citation: parsedCitation } };
  } else if (
    type === "workflow_planned" ||
    type === "workflow_started" ||
    type === "workflow_completed" ||
    type === "workflow_paused" ||
    type === "workflow_resumed" ||
    type === "workflow_cancelled"
  ) {
    const runId = stringField(eventPayload, "runId");
    if (runId) return { ...base, type, payload: { runId } };
  } else if (type === "workflow_step_started" || type === "workflow_step_completed") {
    const runId = stringField(eventPayload, "runId");
    const stepId = stringField(eventPayload, "stepId");
    if (runId && stepId) return { ...base, type, payload: { runId, stepId } };
  } else if (type === "workflow_step_failed") {
    const runId = stringField(eventPayload, "runId");
    const stepId = stringField(eventPayload, "stepId");
    if (runId && stepId && typeof eventPayload.retriable === "boolean") {
      return { ...base, type, payload: { runId, stepId, retriable: eventPayload.retriable } };
    }
  }
  throw new InvalidHarnessPayloadError("HarnessEvent");
}
