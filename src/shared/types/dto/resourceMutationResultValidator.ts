import { isGraphResourcePath } from "./editorProjectionGuards";
import type {
  GraphProjectionReplacementDto,
  ResourceDeltaDto,
  ResourceMutationResultDto,
} from "./editorMutation";
import { areResourceDeltasValid } from "./resourceMutationWireValidator";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function validateUniqueGraphPaths(value: unknown, label: string): string[] | string {
  if (!Array.isArray(value) || !value.every(isGraphResourcePath)) return `${label} are malformed`;
  if (new Set(value).size !== value.length) return `${label} contain duplicates`;
  return value;
}

function graphPathFromDelta(delta: ResourceDeltaDto): string | undefined {
  if (delta.payload.kind === "resource_move" || delta.payload.kind === "resource_lifecycle")
    return undefined;
  return delta.resource.kind === "graph" || delta.resource.kind === "function"
    ? delta.resource.key
    : undefined;
}

function validateReplacement(
  replacement: GraphProjectionReplacementDto,
  deltas: ResourceDeltaDto[],
): string | undefined {
  if (
    !isRecord(replacement) ||
    !isGraphResourcePath(replacement.graphPath) ||
    !isRecord(replacement.projection) ||
    replacement.projection.graphPath !== replacement.graphPath ||
    replacement.projection.basis?.graphPath !== replacement.graphPath ||
    !Number.isSafeInteger(replacement.projection.sourceRevision) ||
    replacement.projection.sourceRevision < 0
  ) {
    return "projection replacement path identity is malformed";
  }
  const graphDelta = deltas.find(
    (candidate) =>
      candidate.resource.kind === "graph" && candidate.resource.key === replacement.graphPath,
  );
  if (graphDelta && replacement.projection.sourceRevision !== graphDelta.toRevision) {
    return `replacement for '${replacement.graphPath}' disagrees with its graph delta`;
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

export function validateResourceMutationWireResult(
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
    !result.moves.every(
      (move) =>
        isRecord(move) &&
        typeof move.from === "string" &&
        move.from.length > 0 &&
        typeof move.to === "string" &&
        move.to.length > 0 &&
        move.from !== move.to &&
        (move.kind === "worksheet" ||
          ((move.kind === "event" || move.kind === "function") &&
            isGraphResourcePath(move.from) &&
            isGraphResourcePath(move.to))) &&
        typeof move.name === "string" &&
        move.name.trim().length > 0,
    )
  )
    return "resource moves are malformed";
  if (!areResourceDeltasValid(result.deltas)) return "resource deltas are malformed";
  if (
    result.deltas.some((delta) => delta.causedBy !== null && delta.causedBy !== result.operationId)
  ) {
    return "resource delta operation correlation is inconsistent";
  }

  if (!Array.isArray(result.projectionReplacements)) {
    return "projection replacements are malformed";
  }
  if (
    typeof result.history?.canUndo !== "boolean" ||
    typeof result.history?.canRedo !== "boolean"
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
