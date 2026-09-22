import { parseGraphResultState } from "./resultParser";
import type {
  EditorGraphMutationDto,
  GraphDocumentDto,
  GraphSaveResultDto,
  GraphEditResultDto,
  GraphEditorSessionDto,
  GraphEditingStateDto,
  GraphEditVersionDto,
  TypeExprDto,
} from "./editorMutation";
import type { GraphProjectionReplacementDto } from "./editorProjection";
import { isRustDataValueWire } from "./dataValue";
import { isBackendDataType } from "../domain/valueType";
import {
  isEditorGraphProjectionDto,
  isFunctionEditorProjectionDto,
  isGraphResourcePath,
  isUuid,
} from "./editorProjectionGuards";

type UnknownRecord = Record<string, unknown>;

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

export function isTypedLiteralWire(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["value_type", "value"]) &&
    isTypeExprWire(value.value_type) &&
    isRustDataValueWire(value.value)
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

export function isGraphConstant(
  value: unknown,
): value is import("../domain/editorMutation").GraphConstantDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      "id",
      "name",
      "dataType",
      "dataValue",
      ...["tabular", "description", "tags"].filter((key) => key in value),
    ])
  )
    return false;
  if (
    !isUuid(value.id) ||
    typeof value.name !== "string" ||
    !value.name.trim() ||
    !isBackendDataType(value.dataType) ||
    !isRustDataValueWire(value.dataValue)
  )
    return false;
  if ("description" in value && typeof value.description !== "string") return false;
  if (
    "tags" in value &&
    (!Array.isArray(value.tags) || !value.tags.every((tag) => typeof tag === "string"))
  )
    return false;
  if ("tabular" in value) {
    if (
      !isRecord(value.tabular) ||
      !hasExactKeys(value.tabular, ["columns"]) ||
      !isRecord(value.tabular.columns)
    )
      return false;
    let length: number | undefined;
    for (const cells of Object.values(value.tabular.columns)) {
      if (
        !Array.isArray(cells) ||
        !cells.every(
          (cell) =>
            cell === null ||
            typeof cell === "boolean" ||
            typeof cell === "string" ||
            (typeof cell === "number" && Number.isFinite(cell)),
        )
      )
        return false;
      if (length !== undefined && cells.length !== length) return false;
      length = cells.length;
    }
  }
  return true;
}

export function parseGraphDocumentDto(value: unknown): GraphDocumentDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      "nodes",
      "port_bindings",
      "connections",
      "input_states",
      ...("constants" in value ? ["constants"] : []),
    ]) ||
    ("constants" in value &&
      (!isRecord(value.constants) ||
        !Object.entries(value.constants).every(
          ([id, constant]) => isUuid(id) && isGraphConstant(constant) && constant.id === id,
        ))) ||
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
  // Validated IPC values are read projections; incremental delivery preserves unchanged references.
  return value as unknown as GraphDocumentDto;
}

export function parseGraphEditorSessionDto(value: unknown): GraphEditorSessionDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["document", "projection", "editing", "resultState"])
  ) {
    throw new Error("Graph editor session is malformed");
  }
  const projection = isEditorGraphProjectionDto(value.projection) ? value.projection : null;
  if (!projection) throw new Error("Graph editor session projection is malformed");
  const resultState = parseGraphResultState(value.resultState);
  if (resultState.semanticInputHash !== projection.basis.semanticInputHash) {
    throw new Error("Graph result state does not match its semantic projection");
  }
  return {
    editing: parseGraphEditingState(value.editing),
    document: parseGraphDocumentDto(value.document),
    projection,
    resultState,
  };
}

export function parseGraphEditResultDto(value: unknown): GraphEditResultDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["changed", "document", "projection", "editing", "resultState"]) ||
    typeof value.changed !== "boolean" ||
    !isEditorGraphProjectionDto(value.projection)
  ) {
    throw new Error("Graph draft transform result is malformed");
  }
  return {
    ...parseGraphEditorSessionDto({
      document: value.document,
      projection: value.projection,
      editing: value.editing,
      resultState: value.resultState,
    }),
    changed: value.changed,
  };
}

export function parseGraphSaveResultDto(
  value: unknown,
  expectedProjectInstanceId: string,
): GraphSaveResultDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      "projectInstanceId",
      "resourceRevision",
      "document",
      "projectionReplacement",
      "editing",
      "resultState",
    ]) ||
    value.projectInstanceId !== expectedProjectInstanceId ||
    !Number.isSafeInteger(value.resourceRevision) ||
    (value.resourceRevision as number) < 0
  ) {
    throw new Error("Graph draft save result is malformed");
  }
  const projectionReplacement = parseGraphProjectionReplacementDto(value.projectionReplacement);
  const session = parseGraphEditorSessionDto({
    document: value.document,
    projection: projectionReplacement.projection,
    editing: value.editing,
    resultState: value.resultState,
  });
  return {
    projectInstanceId: expectedProjectInstanceId,
    editing: session.editing,
    resourceRevision: value.resourceRevision as number,
    document: session.document,
    projectionReplacement,
    resultState: session.resultState,
  };
}

export function parseGraphEditingState(value: unknown): GraphEditingStateDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["version", "dirty", "canUndo", "canRedo"]) ||
    typeof value.dirty !== "boolean" ||
    typeof value.canUndo !== "boolean" ||
    typeof value.canRedo !== "boolean"
  ) {
    throw new Error("Graph editing state is malformed");
  }
  return {
    version: parseGraphEditVersion(value.version),
    dirty: value.dirty,
    canUndo: value.canUndo,
    canRedo: value.canRedo,
  };
}

export function parseGraphEditVersion(value: unknown): GraphEditVersionDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["sessionId", "revision"]) ||
    !isUuid(value.sessionId) ||
    typeof value.revision !== "string" ||
    !/^(0|[1-9][0-9]{0,19})$/u.test(value.revision) ||
    BigInt(value.revision) > 18446744073709551615n
  )
    throw new Error("Graph editing version is malformed");
  return { sessionId: value.sessionId, revision: value.revision };
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
