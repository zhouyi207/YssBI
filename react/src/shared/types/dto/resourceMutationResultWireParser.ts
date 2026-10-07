import { areResourceDeltasValid } from "./resourceMutationWireValidator";
import { isDatabaseEngine } from "@/shared/types/domain/database";
import { isChartDocumentState } from "@/shared/types/domain/chart";
import { validateResourceMutationResult } from "@/shared/types/domain/resourceMutationValidation";

import type {
  ProjectionStatusDto,
  ResourceDeltaDto,
  ResourceDocumentPatchDto,
  ResourceKeyDto,
  ResourceMoveDto,
  ResourceMutationResultDto,
} from "@/shared/types/dto/editorMutation";
import { parseGraphProjectionReplacementDto } from "@/shared/types/dto/editorMutationWireParser";

type UnknownRecord = Record<string, unknown>;

function assertNever(value: never): never {
  throw new Error(`Unhandled resource mutation variant '${String(value)}'`);
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

function isSafeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value);
}

function isFunctionPatchShape(value: unknown): boolean {
  // areResourceDeltasValid already checked both signatures with their strict shared guard.
  return isRecord(value) && hasExactKeys(value, ["before", "after"]);
}

function isPathMoveShape(value: unknown): boolean {
  return isRecord(value) && hasExactKeys(value, ["from", "to"]);
}

function isLifecycleStateShape(value: unknown): boolean {
  return isRecord(value) && hasExactKeys(value, ["revision", "path", "kind", "name"]);
}

function isLifecyclePatchShape(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["before", "after"]) &&
    (value.before === null || isLifecycleStateShape(value.before)) &&
    (value.after === null || isLifecycleStateShape(value.after))
  );
}

function isDatabaseDocumentShape(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["id", "engine", "schemaVersion", "required", "name"]) &&
    isDatabaseEngine(value.engine)
  );
}

function isDatabasePatchShape(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["before", "after"]) &&
    (value.before === null || isDatabaseDocumentShape(value.before)) &&
    (value.after === null || isDatabaseDocumentShape(value.after))
  );
}

function isChartPatchShape(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["before", "after"]) &&
    isChartDocumentState(value.before) &&
    isChartDocumentState(value.after)
  );
}

function isResourcePayloadShape(value: unknown): boolean {
  if (!isRecord(value) || !hasExactKeys(value, ["kind", "patch"])) return false;
  switch (value.kind) {
    case "function":
      return isFunctionPatchShape(value.patch);
    case "chart":
      return isChartPatchShape(value.patch);
    case "resource_lifecycle":
      return isLifecyclePatchShape(value.patch);
    case "resource_move":
      return isPathMoveShape(value.patch);
    case "database":
      return isDatabasePatchShape(value.patch);
    default:
      return false;
  }
}

function hasExactResourceDeltaShape(value: unknown): boolean {
  return (
    isRecord(value) &&
    hasExactKeys(value, ["resource", "fromRevision", "toRevision", "causedBy", "payload"]) &&
    isRecord(value.resource) &&
    hasExactKeys(value.resource, ["kind", "key"]) &&
    isResourcePayloadShape(value.payload)
  );
}

function cloneResourceKey(resource: ResourceKeyDto): ResourceKeyDto {
  switch (resource.kind) {
    case "mind":
      return { kind: "mind", key: resource.key };
    case "doc":
      return { kind: "doc", key: resource.key };
    case "graph":
      return { kind: "graph", key: resource.key };
    case "function":
      return { kind: "function", key: resource.key };
    case "database":
      return { kind: "database", key: resource.key };
    case "chart":
      return { kind: "chart", key: resource.key };
    default:
      return assertNever(resource);
  }
}

function cloneResourcePayload(payload: ResourceDocumentPatchDto): ResourceDocumentPatchDto {
  switch (payload.kind) {
    case "function":
      return { kind: "function", patch: structuredClone(payload.patch) };
    case "chart":
      return { kind: "chart", patch: structuredClone(payload.patch) };
    case "resource_lifecycle":
      return { kind: "resource_lifecycle", patch: structuredClone(payload.patch) };
    case "resource_move":
      return { kind: "resource_move", patch: structuredClone(payload.patch) };
    case "database":
      return { kind: "database", patch: structuredClone(payload.patch) };
    default:
      return assertNever(payload);
  }
}

function parseResourceDeltas(value: unknown): ResourceDeltaDto[] {
  if (!areResourceDeltasValid(value) || !value.every(hasExactResourceDeltaShape)) {
    throw new Error("resource deltas are malformed");
  }
  return value.map((delta) => ({
    resource: cloneResourceKey(delta.resource),
    fromRevision: delta.fromRevision,
    toRevision: delta.toRevision,
    causedBy: delta.causedBy,
    payload: cloneResourcePayload(delta.payload),
  }));
}

function parseMoves(value: unknown): ResourceMoveDto[] {
  if (!Array.isArray(value)) throw new Error("resource moves are malformed");
  return value.map((move) => {
    if (
      !isRecord(move) ||
      !hasExactKeys(move, ["from", "to", "kind", "name"]) ||
      typeof move.from !== "string" ||
      typeof move.to !== "string" ||
      (move.kind !== "event_graph" &&
        move.kind !== "function_graph" &&
        move.kind !== "chart" &&
        move.kind !== "mind" &&
        move.kind !== "doc") ||
      typeof move.name !== "string"
    )
      throw new Error("resource moves are malformed");
    return { from: move.from, to: move.to, kind: move.kind, name: move.name };
  });
}

function parseProjectionStatus(value: unknown): ProjectionStatusDto {
  if (!isRecord(value) || typeof value.status !== "string") {
    throw new Error("projection status is malformed");
  }
  switch (value.status) {
    case "complete":
      if (
        !hasExactKeys(value, ["status", "expectedGraphPaths"]) ||
        !Array.isArray(value.expectedGraphPaths) ||
        !value.expectedGraphPaths.every((path) => typeof path === "string")
      ) {
        throw new Error("projection status is malformed");
      }
      return { status: "complete", expectedGraphPaths: [...value.expectedGraphPaths] };
    case "incomplete":
      if (
        !hasExactKeys(value, ["status", "invalidatedGraphPaths"]) ||
        !Array.isArray(value.invalidatedGraphPaths) ||
        !value.invalidatedGraphPaths.every((path) => typeof path === "string")
      ) {
        throw new Error("projection status is malformed");
      }
      return { status: "incomplete", invalidatedGraphPaths: [...value.invalidatedGraphPaths] };
    default:
      throw new Error("projection status is malformed");
  }
}

export function parseResourceMutationResultDto(value: unknown): ResourceMutationResultDto {
  if (!isRecord(value)) throw new Error("resource mutation result is malformed");
  const keys = [
    "operationId",
    "projectInstanceId",
    "publicationRevision",
    "moves",
    "deltas",
    "projectionReplacements",
    "projectionStatus",
  ];
  if (
    !hasExactKeys(value, keys) ||
    typeof value.operationId !== "string" ||
    typeof value.projectInstanceId !== "string" ||
    !isSafeInteger(value.publicationRevision)
  ) {
    throw new Error("resource mutation result is malformed");
  }

  const result: ResourceMutationResultDto = {
    operationId: value.operationId,
    projectInstanceId: value.projectInstanceId,
    publicationRevision: value.publicationRevision,
    moves: parseMoves(value.moves),
    deltas: parseResourceDeltas(value.deltas),
    projectionReplacements: Array.isArray(value.projectionReplacements)
      ? value.projectionReplacements.map((replacement) =>
          structuredClone(parseGraphProjectionReplacementDto(replacement)),
        )
      : (() => {
          throw new Error("projection replacements are malformed");
        })(),
    projectionStatus: parseProjectionStatus(value.projectionStatus),
  };

  const validationError = validateResourceMutationResult(result);
  if (validationError) throw new Error(validationError);
  return result;
}
