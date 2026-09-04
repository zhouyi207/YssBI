import type { ClipboardSubgraphDto } from "./clipboardSubgraph";
import { isTypedLiteralWire } from "./editorMutationWireParser";

interface UnknownRecord {
  [key: string]: unknown;
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

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === "string";
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function isJsonValue(value: unknown): boolean {
  if (value === null || typeof value === "string" || typeof value === "boolean") return true;
  if (isFiniteNumber(value)) return true;
  if (Array.isArray(value)) return value.every(isJsonValue);
  return isRecord(value) && Object.values(value).every(isJsonValue);
}

function isPosition(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["x", "y"]) &&
    isFiniteNumber(value.x) &&
    isFiniteNumber(value.y)
  );
}

function isTypeExpr(value: unknown): boolean {
  if (value === "Unknown") return true;
  if (!isRecord(value) || Object.keys(value).length !== 1) return false;
  if (hasExactKeys(value, ["Concrete"])) return typeof value.Concrete === "string";
  if (hasExactKeys(value, ["Generic"])) return typeof value.Generic === "string";
  if (hasExactKeys(value, ["Applied"])) {
    return (
      isRecord(value.Applied) &&
      hasExactKeys(value.Applied, ["constructor", "arguments"]) &&
      typeof value.Applied.constructor === "string" &&
      Array.isArray(value.Applied.arguments) &&
      value.Applied.arguments.every(isTypeExpr)
    );
  }
  return (
    hasExactKeys(value, ["Union"]) && Array.isArray(value.Union) && value.Union.every(isTypeExpr)
  );
}

function isCreateArgs(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["kind"]) &&
    ["function", "variable", "database"].includes(value.kind as string)
  );
}

function isNodeCreation(value: unknown): boolean {
  if (!isRecord(value)) return false;
  if (value.kind === "static") {
    return hasExactKeys(value, ["kind", "nodeTypeId"]) && isNonEmptyString(value.nodeTypeId);
  }
  return (
    value.kind === "resourceBound" &&
    hasExactKeys(value, ["kind", "nodeTypeId", "resourcePath", "createArgs"]) &&
    isNonEmptyString(value.nodeTypeId) &&
    isNonEmptyString(value.resourcePath) &&
    isCreateArgs(value.createArgs)
  );
}

function isPortRef(value: unknown): boolean {
  if (!isRecord(value)) return false;
  if (value.kind === "declared") {
    return hasExactKeys(value, ["kind", "key"]) && isNonEmptyString(value.key);
  }
  return (
    value.kind === "instance" &&
    hasExactKeys(value, ["kind", "template", "localInstanceId"]) &&
    isNonEmptyString(value.template) &&
    isNonEmptyString(value.localInstanceId)
  );
}

function isPortAddress(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["nodeId", "port"]) &&
    isNonEmptyString(value.nodeId) &&
    isPortRef(value.port)
  );
}

function isNode(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["localId", "creation", "parameters", "userLabel", "relativePosition"]) &&
    isNonEmptyString(value.localId) &&
    isNodeCreation(value.creation) &&
    isRecord(value.parameters) &&
    isJsonValue(value.parameters) &&
    isNullableString(value.userLabel) &&
    isPosition(value.relativePosition)
  );
}

function isDynamicMemberOrigin(value: unknown): boolean {
  if (!isRecord(value)) return false;
  if (value.kind === "functionParameter") {
    return (
      hasExactKeys(value, ["kind", "function", "parameter"]) &&
      isNonEmptyString(value.function) &&
      isNonEmptyString(value.parameter)
    );
  }
  return (
    value.kind === "schemaField" &&
    hasExactKeys(value, ["kind", "source", "field"]) &&
    isNonEmptyString(value.source) &&
    isNonEmptyString(value.field)
  );
}

function isLastKnown(value: unknown): boolean {
  if (!isRecord(value) || typeof value.label !== "string") return false;
  return (
    hasExactKeys(value, ["label"]) ||
    (hasExactKeys(value, ["label", "valueType"]) && isTypeExpr(value.valueType))
  );
}

function isDynamicPortBinding(value: unknown): boolean {
  if (!isRecord(value)) return false;
  if (value.kind === "userCreated") {
    return hasExactKeys(value, ["kind", "order"]) && typeof value.order === "string";
  }
  return (
    (value.kind === "resolved" || value.kind === "orphan") &&
    hasExactKeys(value, ["kind", "origin", "order", "lastKnown"]) &&
    isDynamicMemberOrigin(value.origin) &&
    typeof value.order === "string" &&
    isLastKnown(value.lastKnown)
  );
}

function isPortBinding(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["address", "binding"]) &&
    isPortAddress(value.address) &&
    isDynamicPortBinding(value.binding)
  );
}

function isInputState(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["address", "state"]) &&
    isPortAddress(value.address) &&
    isRecord(value.state) &&
    hasExactKeys(value.state, ["literalOverride"]) &&
    (value.state.literalOverride === null || isTypedLiteralWire(value.state.literalOverride))
  );
}

function isConnection(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["output", "input", "order"]) &&
    isPortAddress(value.output) &&
    isPortAddress(value.input) &&
    isNullableString(value.order)
  );
}

export function parseClipboardSubgraphDto(value: unknown): ClipboardSubgraphDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      "schemaVersion",
      "nodes",
      "portBindings",
      "inputStates",
      "connections",
    ]) ||
    value.schemaVersion !== 1 ||
    !Array.isArray(value.nodes) ||
    !value.nodes.every(isNode) ||
    !Array.isArray(value.portBindings) ||
    !value.portBindings.every(isPortBinding) ||
    !Array.isArray(value.inputStates) ||
    !value.inputStates.every(isInputState) ||
    !Array.isArray(value.connections) ||
    !value.connections.every(isConnection)
  ) {
    throw new Error("Invalid clipboard subgraph response");
  }

  return value as unknown as ClipboardSubgraphDto;
}
