import type { ResourceMutationResultDto } from "@/shared/types/domain/editorMutation";
import { isGraphResourcePath } from "@/shared/types/domain/editorProjectionGuards";

type UnknownRecord = Record<string, unknown>;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function canonicalize(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (isRecord(value)) {
    return Object.fromEntries(
      Object.entries(value)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, entry]) => [key, canonicalize(entry)]),
    );
  }
  return value;
}

export function collectResourceMutationGraphPaths(
  result: unknown,
  fallbackPaths: Iterable<string> = [],
): Set<string> {
  const paths = new Set(fallbackPaths);
  if (!isRecord(result)) return paths;
  const moves = Array.isArray(result.moves) ? result.moves : [result.moves];
  for (const move of moves) {
    if (!isRecord(move) || (move.kind !== "event" && move.kind !== "function")) continue;
    if (isGraphResourcePath(move.from)) paths.add(move.from);
    if (isGraphResourcePath(move.to)) paths.add(move.to);
  }
  const deltas = Array.isArray(result.deltas) ? result.deltas : [result.deltas];
  for (const delta of deltas) {
    if (!isRecord(delta) || !isRecord(delta.resource)) continue;
    if (
      (delta.resource.kind === "graph" || delta.resource.kind === "function") &&
      isGraphResourcePath(delta.resource.key)
    )
      paths.add(delta.resource.key);
  }
  const replacements = Array.isArray(result.projectionReplacements)
    ? result.projectionReplacements
    : [result.projectionReplacements];
  for (const replacement of replacements) {
    if (isRecord(replacement) && isGraphResourcePath(replacement.graphPath)) {
      paths.add(replacement.graphPath);
    }
  }
  if (isRecord(result.projectionStatus)) {
    for (const key of ["expectedGraphPaths", "invalidatedGraphPaths"] as const) {
      const declared = result.projectionStatus[key];
      const entries = Array.isArray(declared) ? declared : [declared];
      for (const path of entries) if (isGraphResourcePath(path)) paths.add(path);
    }
  }
  return paths;
}

export function fingerprintResourceMutationResult(result: ResourceMutationResultDto): string {
  return JSON.stringify(canonicalize(result));
}
