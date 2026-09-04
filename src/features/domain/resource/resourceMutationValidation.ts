import { isGraphResourcePath } from "@/shared/types/domain/editorProjectionGuards";
import type {
  GraphProjectionReplacementDto,
  ResourceDeltaDto,
  ResourceDocumentPatchDto,
  ResourceMutationResultDto,
} from "@/shared/types/domain/editorMutation";

type UnknownRecord = Record<string, unknown>;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isSafeRevision(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

function isResourceKind(value: unknown): value is ResourceDeltaDto["resource"]["kind"] {
  return (
    value === "graph" ||
    value === "function" ||
    value === "variable" ||
    value === "database" ||
    value === "chart"
  );
}

function isPayloadKind(value: unknown): value is ResourceDocumentPatchDto["kind"] {
  return (
    value === "graph" ||
    value === "function" ||
    value === "chart" ||
    value === "resource_lifecycle" ||
    value === "resource_move" ||
    value === "variable" ||
    value === "variable_scope_move" ||
    value === "database"
  );
}

function isResourceDelta(value: unknown): value is ResourceDeltaDto {
  if (
    !isRecord(value) ||
    !isRecord(value.resource) ||
    !isResourceKind(value.resource.kind) ||
    typeof value.resource.key !== "string" ||
    value.resource.key.length === 0 ||
    !isSafeRevision(value.fromRevision) ||
    !isSafeRevision(value.toRevision) ||
    (value.causedBy !== null && typeof value.causedBy !== "string") ||
    !isRecord(value.payload) ||
    !isPayloadKind(value.payload.kind) ||
    !("patch" in value.payload)
  ) {
    return false;
  }
  return value.toRevision >= value.fromRevision;
}

function graphPathFromDelta(delta: ResourceDeltaDto): string | undefined {
  if (delta.payload.kind === "resource_move" || delta.payload.kind === "resource_lifecycle")
    return undefined;
  return delta.resource.kind === "graph" || delta.resource.kind === "function"
    ? delta.resource.key
    : undefined;
}

function validateUniqueGraphPaths(value: unknown, label: string): string[] | string {
  if (!Array.isArray(value) || !value.every(isGraphResourcePath)) return `${label} are malformed`;
  if (new Set(value).size !== value.length) return `${label} contain duplicates`;
  return value;
}

function validateReplacement(
  replacement: GraphProjectionReplacementDto,
  deltas: readonly ResourceDeltaDto[],
): string | undefined {
  if (
    !isRecord(replacement) ||
    !isGraphResourcePath(replacement.graphPath) ||
    !isRecord(replacement.projection) ||
    replacement.projection.graphPath !== replacement.graphPath ||
    replacement.projection.basis?.graphPath !== replacement.graphPath
  ) {
    return "projection replacement path identity is malformed";
  }
  const functionDelta = deltas.find(
    (candidate) =>
      candidate.resource.kind === "function" && candidate.resource.key === replacement.graphPath,
  );
  if (
    functionDelta &&
    (!("functionEditorProjection" in replacement) ||
      replacement.functionEditorProjection?.functionRevision !== functionDelta.toRevision)
  ) {
    return `replacement for '${replacement.graphPath}' disagrees with its function delta`;
  }
  return undefined;
}

/** Validates the typed Application receipt after the Services wire parser. */
export function validateResourceMutationResult(
  result: ResourceMutationResultDto,
): string | undefined {
  if (!isRecord(result)) return "resource mutation result is malformed";
  if (typeof result.operationId !== "string" || !result.operationId) {
    return "operation correlation is malformed";
  }
  if (typeof result.projectInstanceId !== "string" || !result.projectInstanceId) {
    return "project instance identity is malformed";
  }
  if (!Number.isSafeInteger(result.publicationRevision) || result.publicationRevision < 1) {
    return "publication revision is malformed";
  }
  if (
    !Array.isArray(result.moves) ||
    result.moves.some((move) => {
      if (
        !isRecord(move) ||
        typeof move.from !== "string" ||
        move.from.length === 0 ||
        typeof move.to !== "string" ||
        move.to.length === 0 ||
        move.from === move.to ||
        typeof move.name !== "string" ||
        move.name.trim().length === 0
      )
        return true;
      if (move.kind === "chart") return false;
      if (move.kind !== "event" && move.kind !== "function") return true;
      return !isGraphResourcePath(move.from) || !isGraphResourcePath(move.to);
    })
  )
    return "resource moves are malformed";
  if (!Array.isArray(result.deltas) || !result.deltas.every(isResourceDelta)) {
    return "resource deltas are malformed";
  }
  if (
    new Set(result.deltas.map((delta) => `${delta.resource.kind}:${delta.resource.key}`)).size !==
    result.deltas.length
  ) {
    return "resource deltas contain duplicate targets";
  }
  if (
    result.deltas.some((delta) => delta.causedBy !== null && delta.causedBy !== result.operationId)
  ) {
    return "resource delta operation correlation is inconsistent";
  }
  if (!Array.isArray(result.projectionReplacements)) {
    return "projection replacements are malformed";
  }
  if (
    !isRecord(result.history) ||
    typeof result.history.canUndo !== "boolean" ||
    typeof result.history.canRedo !== "boolean"
  ) {
    return "history status is malformed";
  }

  let expectedPaths: string[] | undefined;
  if (result.projectionStatus?.status === "complete") {
    const validated = validateUniqueGraphPaths(
      result.projectionStatus.expectedGraphPaths,
      "expected graph paths",
    );
    if (typeof validated === "string") return validated;
    expectedPaths = validated;
  } else if (result.projectionStatus?.status === "incomplete") {
    const validated = validateUniqueGraphPaths(
      result.projectionStatus.invalidatedGraphPaths,
      "invalidated graph paths",
    );
    if (typeof validated === "string") return validated;
  } else {
    return "projection status is malformed";
  }

  const replacementPaths = new Set<string>();
  for (const replacement of result.projectionReplacements) {
    const error = validateReplacement(replacement, result.deltas);
    if (error) return error;
    if (replacementPaths.has(replacement.graphPath)) {
      return `duplicate replacement for '${replacement.graphPath}'`;
    }
    replacementPaths.add(replacement.graphPath);
  }

  if (expectedPaths) {
    const expected = new Set(expectedPaths);
    if (
      expected.size !== replacementPaths.size ||
      [...expected].some((path) => !replacementPaths.has(path))
    ) {
      return "complete replacement paths do not equal the declared expected graph paths";
    }
    for (const delta of result.deltas) {
      const path = graphPathFromDelta(delta);
      if (path && !expected.has(path)) {
        return `delta path '${path}' is absent from the declared expected graph paths`;
      }
    }
  }
  return undefined;
}
