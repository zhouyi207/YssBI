import type {
  CompileGraphDraftDto,
  EditorGraphMutationDto,
  GraphDocumentDto,
  GraphDraftSaveDto,
  GraphDraftTransformDto,
  GraphEditorSessionDto,
  HistoryStatusDto,
  TypeExprDto,
} from "./editorMutation";
import type { GraphProjectionReplacementDto } from "./editorProjection";
import {
  isEditorGraphProjectionDto,
  isFunctionEditorProjectionDto,
  isGraphResourcePath,
  isUuid,
} from "./editorProjectionGuards";

type UnknownRecord = Record<string, unknown>;
const SHA256_HEX_PATTERN = /^[0-9a-f]{64}$/;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, keys: readonly string[]): boolean {
  return (
    Object.keys(value).length === keys.length &&
    keys.every((key) => Object.prototype.hasOwnProperty.call(value, key))
  );
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === "string";
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function isPosition(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["x", "y"]) &&
    isFiniteNumber(value.x) &&
    isFiniteNumber(value.y)
  );
}

export function parseEditorGraphMutationDto(
  value: unknown,
): Extract<EditorGraphMutationDto, { type: "insertReroute" }> {
  if (
    !isRecord(value) ||
    value.type !== "insertReroute" ||
    !hasExactKeys(value, ["type", "payload"]) ||
    !isRecord(value.payload) ||
    !hasExactKeys(value.payload, ["connectionId", "position"]) ||
    typeof value.payload.connectionId !== "string" ||
    value.payload.connectionId.trim().length === 0 ||
    !isPosition(value.payload.position)
  ) {
    throw new Error(
      "InsertReroute mutation must have exact connectionId and finite position fields",
    );
  }

  return {
    type: "insertReroute",
    payload: {
      connectionId: value.payload.connectionId,
      position: {
        x: (value.payload.position as { x: number }).x,
        y: (value.payload.position as { y: number }).y,
      },
    },
  };
}

function isDocumentPortAddress(value: unknown): boolean {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["node_id", "port"]) ||
    !isUuid(value.node_id) ||
    !isRecord(value.port)
  )
    return false;
  if (value.port.kind === "declared") {
    return hasExactKeys(value.port, ["kind", "key"]) && typeof value.port.key === "string";
  }
  return (
    value.port.kind === "instance" &&
    hasExactKeys(value.port, ["kind", "template", "instance_id"]) &&
    typeof value.port.template === "string" &&
    isUuid(value.port.instance_id)
  );
}

function isDocumentNode(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["id", "node_type", "position", "parameters", "user_label"]) &&
    isUuid(value.id) &&
    typeof value.node_type === "string" &&
    isPosition(value.position) &&
    isRecord(value.parameters) &&
    isNullableString(value.user_label)
  );
}

function isDynamicMemberLocator(value: unknown): boolean {
  if (!isRecord(value)) return false;
  if (value.kind === "function_parameter") {
    return (
      hasExactKeys(value, ["kind", "function", "parameter"]) &&
      typeof value.function === "string" &&
      typeof value.parameter === "string"
    );
  }
  return (
    value.kind === "schema_field" &&
    hasExactKeys(value, ["kind", "source", "field"]) &&
    typeof value.source === "string" &&
    typeof value.field === "string"
  );
}

export function isTypeExprWire(value: unknown): value is TypeExprDto {
  if (value === "Unknown") return true;
  if (!isRecord(value) || Object.keys(value).length !== 1) return false;
  if (hasExactKeys(value, ["Concrete"])) return typeof value.Concrete === "string";
  if (hasExactKeys(value, ["Class"])) return typeof value.Class === "string";
  if (hasExactKeys(value, ["Generic"])) return typeof value.Generic === "string";
  if (hasExactKeys(value, ["Applied"])) {
    return (
      isRecord(value.Applied) &&
      hasExactKeys(value.Applied, ["constructor", "arguments"]) &&
      typeof value.Applied.constructor === "string" &&
      Array.isArray(value.Applied.arguments) &&
      value.Applied.arguments.every(isTypeExprWire)
    );
  }
  return (
    hasExactKeys(value, ["Union"]) &&
    Array.isArray(value.Union) &&
    value.Union.every(isTypeExprWire)
  );
}

function isCanonicalDecimal(value: unknown): value is string {
  return (
    typeof value === "string" && /^-?(?:0|[1-9]\d*)(?:\.\d*[1-9])?$/.test(value) && value !== "-0"
  );
}

export function isProtocolValueWire(value: unknown): boolean {
  if (value === "Null") return true;
  if (!isRecord(value) || Object.keys(value).length !== 1) return false;
  if ("Bool" in value) return typeof value.Bool === "boolean";
  if ("Integer" in value) return Number.isSafeInteger(value.Integer);
  if ("Unsigned" in value) {
    return (
      typeof value.Unsigned === "number" &&
      Number.isSafeInteger(value.Unsigned) &&
      value.Unsigned >= 0
    );
  }
  if ("Decimal" in value) return isCanonicalDecimal(value.Decimal);
  if ("String" in value) return typeof value.String === "string";
  if ("Bytes" in value) {
    return (
      Array.isArray(value.Bytes) &&
      value.Bytes.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255)
    );
  }
  if ("List" in value) {
    return Array.isArray(value.List) && value.List.every(isProtocolValueWire);
  }
  return (
    "Object" in value &&
    isRecord(value.Object) &&
    Object.values(value.Object).every(isProtocolValueWire)
  );
}

export function isTypedLiteralWire(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["value_type", "value"]) &&
    isTypeExprWire(value.value_type) &&
    isProtocolValueWire(value.value)
  );
}

function isLastKnownPortMetadata(value: unknown): boolean {
  if (!isRecord(value) || typeof value.label !== "string") return false;
  return (
    hasExactKeys(value, ["label"]) ||
    (hasExactKeys(value, ["label", "value_type"]) && isTypeExprWire(value.value_type))
  );
}

function isDynamicPortBinding(value: unknown): boolean {
  if (!isRecord(value) || typeof value.order !== "string") return false;
  if (value.kind === "user_created") return hasExactKeys(value, ["kind", "order"]);
  if (value.kind === "resolved") {
    return (
      hasExactKeys(value, ["kind", "origin", "order", "last_known"]) &&
      isDynamicMemberLocator(value.origin) &&
      isLastKnownPortMetadata(value.last_known)
    );
  }
  return (
    value.kind === "orphan" &&
    hasExactKeys(value, ["kind", "origin", "order", "last_known"]) &&
    isDynamicMemberLocator(value.origin) &&
    isLastKnownPortMetadata(value.last_known)
  );
}

function isDocumentConnection(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["id", "output", "input", "order"]) &&
    isUuid(value.id) &&
    isDocumentPortAddress(value.output) &&
    isDocumentPortAddress(value.input) &&
    isNullableString(value.order)
  );
}

function isInputState(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["literal_override"]) &&
    (value.literal_override === null || isTypedLiteralWire(value.literal_override))
  );
}

export function parseGraphDocumentDto(value: unknown): GraphDocumentDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["nodes", "port_bindings", "connections", "input_states"]) ||
    !isRecord(value.nodes) ||
    !Object.entries(value.nodes).every(
      ([nodeId, node]) =>
        isUuid(nodeId) && isDocumentNode(node) && (node as { id: string }).id === nodeId,
    ) ||
    !Array.isArray(value.port_bindings) ||
    !value.port_bindings.every(
      (entry) =>
        Array.isArray(entry) &&
        entry.length === 2 &&
        isDocumentPortAddress(entry[0]) &&
        isDynamicPortBinding(entry[1]),
    ) ||
    !isRecord(value.connections) ||
    !Object.entries(value.connections).every(
      ([connectionId, connection]) =>
        isUuid(connectionId) &&
        isDocumentConnection(connection) &&
        (connection as { id: string }).id === connectionId,
    ) ||
    !Array.isArray(value.input_states) ||
    !value.input_states.every(
      (entry) =>
        Array.isArray(entry) &&
        entry.length === 2 &&
        isDocumentPortAddress(entry[0]) &&
        isInputState(entry[1]),
    )
  ) {
    throw new Error("Graph draft document is malformed");
  }
  return structuredClone(value) as unknown as GraphDocumentDto;
}

export function parseGraphEditorSessionDto(value: unknown): GraphEditorSessionDto {
  if (!isRecord(value) || !hasExactKeys(value, ["document", "projection"])) {
    throw new Error("Graph editor session is malformed");
  }
  const projection = isEditorGraphProjectionDto(value.projection) ? value.projection : null;
  if (!projection) throw new Error("Graph editor session projection is malformed");
  return {
    document: parseGraphDocumentDto(value.document),
    projection,
  };
}

export function parseCompileGraphDraftDto(value: unknown): CompileGraphDraftDto {
  if (!isRecord(value) || !isEditorGraphProjectionDto(value.projection)) {
    throw new Error("Graph draft compilation result is malformed");
  }
  if (
    value.type === "blocked" &&
    hasExactKeys(value, ["type", "projection"]) &&
    value.projection.outcome.type === "analysisBlocked" &&
    value.projection.hasBlockingDiagnostics
  ) {
    return { type: "blocked", projection: value.projection };
  }
  if (
    value.type === "ready" &&
    hasExactKeys(value, ["type", "artifactId", "cacheHit", "projection"]) &&
    typeof value.artifactId === "string" &&
    SHA256_HEX_PATTERN.test(value.artifactId) &&
    typeof value.cacheHit === "boolean" &&
    value.projection.outcome.type === "success" &&
    !value.projection.hasBlockingDiagnostics
  ) {
    return {
      type: "ready",
      artifactId: value.artifactId,
      cacheHit: value.cacheHit,
      projection: value.projection,
    };
  }
  throw new Error("Graph draft compilation outcome is malformed");
}

export function parseGraphDraftTransformDto(value: unknown): GraphDraftTransformDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["changed", "document", "projection"]) ||
    typeof value.changed !== "boolean" ||
    !isEditorGraphProjectionDto(value.projection)
  ) {
    throw new Error("Graph draft transform result is malformed");
  }
  return {
    changed: value.changed,
    document: parseGraphDocumentDto(value.document),
    projection: value.projection,
  };
}

export function parseGraphDraftSaveDto(
  value: unknown,
  expectedProjectInstanceId: string,
): GraphDraftSaveDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      "projectInstanceId",
      "operationId",
      "resourceRevision",
      "document",
      "projectionReplacement",
      "history",
    ]) ||
    value.projectInstanceId !== expectedProjectInstanceId ||
    !isUuid(value.operationId) ||
    !Number.isSafeInteger(value.resourceRevision) ||
    (value.resourceRevision as number) < 0
  ) {
    throw new Error("Graph draft save result is malformed");
  }
  return {
    projectInstanceId: expectedProjectInstanceId,
    operationId: value.operationId,
    resourceRevision: value.resourceRevision as number,
    document: parseGraphDocumentDto(value.document),
    projectionReplacement: parseGraphProjectionReplacementDto(value.projectionReplacement),
    history: parseHistoryStatusDto(value.history),
  };
}

export function parseGraphProjectionReplacementDto(value: unknown): GraphProjectionReplacementDto {
  if (
    !isRecord(value) ||
    !isGraphResourcePath(value.graphPath) ||
    !isEditorGraphProjectionDto(value.projection) ||
    value.projection.graphPath !== value.graphPath ||
    value.projection.basis.graphPath !== value.graphPath
  ) {
    throw new Error("Graph mutation projection replacement is malformed");
  }
  if (value.graphPath.startsWith("events/")) {
    if (!hasExactKeys(value, ["graphPath", "projection"])) {
      throw new Error("Graph mutation projection replacement is malformed");
    }
    return { graphPath: value.graphPath, projection: value.projection };
  }
  if (
    !hasExactKeys(value, ["graphPath", "projection", "functionEditorProjection"]) ||
    !isFunctionEditorProjectionDto(value.functionEditorProjection)
  ) {
    throw new Error("Graph mutation projection replacement is malformed");
  }
  return {
    graphPath: value.graphPath,
    projection: value.projection,
    functionEditorProjection: value.functionEditorProjection,
  };
}

export function parseHistoryStatusDto(value: unknown): HistoryStatusDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["canUndo", "canRedo"]) ||
    typeof value.canUndo !== "boolean" ||
    typeof value.canRedo !== "boolean"
  ) {
    throw new Error("Graph mutation history is malformed");
  }
  return { canUndo: value.canUndo, canRedo: value.canRedo };
}
