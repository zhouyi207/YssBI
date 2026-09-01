type UnknownRecord = Record<string, unknown>;

const POSITIVE_DECIMAL_ID_PATTERN = /^[1-9]\d*$/;

export interface StagedRunIdentity {
  projectSessionId: string;
  graphPath: string;
  runId: string;
}

export interface ResultObservationSourceDto {
  graphPath: string;
  nodeId: string | null;
  portAddress: string | null;
}

export type ResultInspectionRequestedRunEvent = {
  type: "resultInspectionRequested";
  resultId: string;
  source: ResultObservationSourceDto;
};

export type StagedResultInspectionRunEventKind =
  | ResultInspectionRequestedRunEvent
  | { type: "runCompleted" };

export interface StagedResultInspectionRunEvent {
  run: StagedRunIdentity;
  kind: StagedResultInspectionRunEventKind;
}

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, keys: readonly string[]): boolean {
  return (
    Object.keys(value).length === keys.length &&
    keys.every((key) => Object.prototype.hasOwnProperty.call(value, key))
  );
}

function fail(): never {
  throw new Error("Invalid staged result inspection run event");
}

function isOpaqueIdentity(value: unknown): value is string {
  return typeof value === "string" && value.length > 0;
}

function isPositiveDecimalId(value: unknown): value is string {
  return typeof value === "string" && POSITIVE_DECIMAL_ID_PATTERN.test(value);
}

function parseRunIdentity(value: unknown): StagedRunIdentity {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["projectSessionId", "graphPath", "runId"]) ||
    !isOpaqueIdentity(value.projectSessionId) ||
    !isOpaqueIdentity(value.graphPath) ||
    !isPositiveDecimalId(value.runId)
  ) {
    return fail();
  }

  return {
    projectSessionId: value.projectSessionId,
    graphPath: value.graphPath,
    runId: value.runId,
  };
}

function parseNullableOpaqueIdentity(value: unknown): string | null {
  if (value === null) return null;
  if (isOpaqueIdentity(value)) return value;
  return fail();
}

function parseObservationSource(value: unknown): ResultObservationSourceDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["graphPath", "nodeId", "portAddress"]) ||
    !isOpaqueIdentity(value.graphPath)
  ) {
    return fail();
  }

  return {
    graphPath: value.graphPath,
    nodeId: parseNullableOpaqueIdentity(value.nodeId),
    portAddress: parseNullableOpaqueIdentity(value.portAddress),
  };
}

function parseKind(value: unknown): StagedResultInspectionRunEventKind {
  if (!isRecord(value) || typeof value.type !== "string") return fail();

  if (value.type === "runCompleted") {
    if (!hasExactKeys(value, ["type"])) return fail();
    return { type: "runCompleted" };
  }

  if (
    value.type !== "resultInspectionRequested" ||
    !hasExactKeys(value, ["type", "resultId", "source"]) ||
    !isPositiveDecimalId(value.resultId)
  ) {
    return fail();
  }

  return {
    type: "resultInspectionRequested",
    resultId: value.resultId,
    source: parseObservationSource(value.source),
  };
}

export function parseStagedResultInspectionRunEvent(
  value: unknown,
): StagedResultInspectionRunEvent {
  if (!isRecord(value) || !hasExactKeys(value, ["run", "kind"])) return fail();

  return {
    run: parseRunIdentity(value.run),
    kind: parseKind(value.kind),
  };
}
