export type HarnessCapabilityId =
  | "inspect_graph"
  | "search_node_catalog"
  | "inspect_dataset_schema"
  | "inspect_dataset_profile"
  | "inspect_result"
  | "inspect_project";

export type WorkflowRunState =
  | "planned"
  | "waiting_for_approval"
  | "ready"
  | "running"
  | "paused"
  | "waiting_for_external_input"
  | "completed"
  | "failed"
  | "cancelled";

export interface HarnessKnowledgeCitation {
  readonly sourceId: string;
  readonly documentId: string;
  readonly chunkId: string;
  readonly title: string;
  readonly version: string;
  readonly sourceHash: string;
}

export interface HarnessMemoryRecord {
  readonly recordId: string;
  readonly scope: "session" | "project" | "user" | "episodic";
  readonly kind:
    | "research_question"
    | "dataset_semantic"
    | "variable_role"
    | "study_design"
    | "method_decision"
    | "model_decision"
    | "user_preference"
    | "reporting_preference"
    | "workflow_summary";
  readonly status: "proposed" | "approved" | "active" | "superseded" | "invalidated" | "deleted";
  readonly value: Readonly<Record<string, unknown>>;
  readonly createdAt: number;
  readonly updatedAt: number;
}

export type HarnessEvent = Readonly<{
  sequence: number;
  sessionId: string;
  turnId: string | null;
  occurredAt: number;
}> &
  (
    | Readonly<{ type: "session_created" | "session_closed" | "turn_failed" | "turn_cancelled" }>
    | Readonly<{ type: "turn_started"; payload: { userMessage: string } }>
    | Readonly<{ type: "text_delta"; payload: { delta: string } }>
    | Readonly<{ type: "plan_proposed"; payload: { plan: unknown } }>
    | Readonly<{
        type: "tool_invocation_requested";
        payload: { capabilityId: HarnessCapabilityId };
      }>
    | Readonly<{
        type: "tool_invocation_started" | "tool_invocation_completed";
        payload: { invocationId: string; capabilityId: HarnessCapabilityId };
      }>
    | Readonly<{ type: "turn_completed"; payload: { finalText: string } }>
    | Readonly<{ type: "knowledge_cited"; payload: { citation: HarnessKnowledgeCitation } }>
    | Readonly<{ type: "memory_recorded"; payload: { record: HarnessMemoryRecord } }>
    | Readonly<{ type: "memory_deleted"; payload: { recordId: string } }>
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

export interface HarnessRuntimeStatus {
  readonly providerConfigured: boolean;
}

export interface HarnessSession {
  readonly sessionId: string;
  readonly projectInstanceId: string;
  readonly projectSessionId: string;
}

export interface HarnessTurnResult {
  readonly finalText: string;
}

export interface HarnessSubscriptionSnapshot {
  readonly subscriptionId: string;
}

export interface HarnessWorkflowRun {
  readonly runId: string;
  readonly state: WorkflowRunState;
}

export class InvalidHarnessPayloadError extends Error {
  constructor(readonly payloadName: string) {
    super(`Invalid ${payloadName} payload`);
    this.name = "InvalidHarnessPayloadError";
  }
}

const CAPABILITY_IDS = new Set<HarnessCapabilityId>([
  "inspect_graph",
  "search_node_catalog",
  "inspect_dataset_schema",
  "inspect_dataset_profile",
  "inspect_result",
  "inspect_project",
]);

const WORKFLOW_STATES = new Set<WorkflowRunState>([
  "planned",
  "waiting_for_approval",
  "ready",
  "running",
  "paused",
  "waiting_for_external_input",
  "completed",
  "failed",
  "cancelled",
]);

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

function capability(value: unknown): HarnessCapabilityId | null {
  return typeof value === "string" && CAPABILITY_IDS.has(value as HarnessCapabilityId)
    ? (value as HarnessCapabilityId)
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

const MEMORY_SCOPES = new Set(["session", "project", "user", "episodic"] as const);
const MEMORY_KINDS = new Set([
  "research_question",
  "dataset_semantic",
  "variable_role",
  "study_design",
  "method_decision",
  "model_decision",
  "user_preference",
  "reporting_preference",
  "workflow_summary",
] as const);
const MEMORY_STATUSES = new Set([
  "proposed",
  "approved",
  "active",
  "superseded",
  "invalidated",
  "deleted",
] as const);

export function parseHarnessMemoryRecord(value: unknown): HarnessMemoryRecord {
  const source = record(value);
  const recordId = source && stringField(source, "recordId");
  const memoryValue = source && record(source.value);
  const createdAt = source && integerField(source, "createdAt");
  const updatedAt = source && integerField(source, "updatedAt");
  if (
    !source ||
    !recordId ||
    !MEMORY_SCOPES.has(source.scope as never) ||
    !MEMORY_KINDS.has(source.kind as never) ||
    !MEMORY_STATUSES.has(source.status as never) ||
    !memoryValue ||
    createdAt === null ||
    updatedAt === null
  ) {
    throw new InvalidHarnessPayloadError("HarnessMemoryRecord");
  }
  return {
    recordId,
    scope: source.scope as HarnessMemoryRecord["scope"],
    kind: source.kind as HarnessMemoryRecord["kind"],
    status: source.status as HarnessMemoryRecord["status"],
    value: memoryValue,
    createdAt,
    updatedAt,
  };
}

export function parseHarnessMemoryRecords(value: unknown): readonly HarnessMemoryRecord[] {
  if (!Array.isArray(value)) throw new InvalidHarnessPayloadError("HarnessMemoryRecords");
  return value.map(parseHarnessMemoryRecord);
}

export function parseHarnessRuntimeStatus(value: unknown): HarnessRuntimeStatus {
  const source = record(value);
  if (!source || typeof source.providerConfigured !== "boolean") {
    throw new InvalidHarnessPayloadError("HarnessRuntimeStatus");
  }
  return { providerConfigured: source.providerConfigured };
}

export function parseHarnessSession(value: unknown): HarnessSession {
  const source = record(value);
  const sessionId = source && stringField(source, "sessionId");
  const projectInstanceValue = source && stringField(source, "projectInstanceId");
  const projectSessionId = source && stringField(source, "projectSessionId");
  if (!sessionId || !projectInstanceValue || !projectSessionId) {
    throw new InvalidHarnessPayloadError("HarnessSession");
  }
  return { sessionId, projectInstanceId: projectInstanceValue, projectSessionId };
}

export function parseHarnessTurnResult(value: unknown): HarnessTurnResult {
  const source = record(value);
  const finalText = source && stringField(source, "finalText");
  if (!finalText) throw new InvalidHarnessPayloadError("HarnessTurnResult");
  return { finalText };
}

export function parseHarnessSubscription(value: unknown): HarnessSubscriptionSnapshot {
  const source = record(value);
  const subscriptionId = source && stringField(source, "subscriptionId");
  if (!subscriptionId) throw new InvalidHarnessPayloadError("HarnessSubscription");
  return { subscriptionId };
}

export function parseHarnessWorkflowRun(value: unknown): HarnessWorkflowRun {
  const source = record(value);
  const runId = source && stringField(source, "runId");
  const state = source?.state;
  if (!runId || typeof state !== "string" || !WORKFLOW_STATES.has(state as WorkflowRunState)) {
    throw new InvalidHarnessPayloadError("HarnessWorkflowRun");
  }
  return { runId, state: state as WorkflowRunState };
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
  if (["session_created", "session_closed", "turn_failed", "turn_cancelled"].includes(type)) {
    return { ...base, type } as HarnessEvent;
  }
  const eventPayload = payload(source);
  if (!eventPayload) throw new InvalidHarnessPayloadError("HarnessEvent");
  if (type === "turn_started") {
    const userMessage = stringField(eventPayload, "userMessage");
    if (userMessage) return { ...base, type, payload: { userMessage } };
  } else if (type === "text_delta") {
    const delta = eventPayload.delta;
    if (typeof delta === "string") return { ...base, type, payload: { delta } };
  } else if (type === "plan_proposed" && record(eventPayload.plan)) {
    return { ...base, type, payload: { plan: eventPayload.plan } };
  } else if (type === "tool_invocation_requested") {
    const capabilityId = capability(eventPayload.capabilityId);
    if (capabilityId) return { ...base, type, payload: { capabilityId } };
  } else if (type === "tool_invocation_started" || type === "tool_invocation_completed") {
    const invocationId = stringField(eventPayload, "invocationId");
    const capabilityId = capability(eventPayload.capabilityId);
    if (invocationId && capabilityId) {
      return { ...base, type, payload: { invocationId, capabilityId } };
    }
  } else if (type === "turn_completed") {
    const finalText = eventPayload.finalText;
    if (typeof finalText === "string") return { ...base, type, payload: { finalText } };
  } else if (type === "knowledge_cited") {
    const parsedCitation = citation(eventPayload.citation);
    if (parsedCitation) return { ...base, type, payload: { citation: parsedCitation } };
  } else if (type === "memory_recorded") {
    try {
      return { ...base, type, payload: { record: parseHarnessMemoryRecord(eventPayload.record) } };
    } catch {
      // Fall through to the common malformed-event error.
    }
  } else if (type === "memory_deleted") {
    const recordId = stringField(eventPayload, "recordId");
    if (recordId) return { ...base, type, payload: { recordId } };
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
