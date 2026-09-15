import { portAddressKey } from "@/features/domain/editorProjection";
import type { RunFailureProjection } from "@/features/core/execution/executionTypes";
import type { GraphDraftSession } from "@/features/core/graphDraft";
import type { GraphResultCacheProjection } from "./runtime";
import type { ConnectionCacheState } from "@/shared/types/domain/result";

export type GraphCacheAppearance = "new" | "stale" | "valid" | "partial";
export type GraphElementState =
  | "uncompiled"
  | "compiling"
  | "compiled"
  | "running"
  | "error"
  | "valid"
  | "stale"
  | "partial";

/** Aggregate read-only cache facts once per canvas, without inventing execution progress. */
export function projectGraphPresentation(
  cache: GraphResultCacheProjection | undefined,
  compileStatus: GraphDraftSession["compileStatus"],
  runningNodeIds: readonly string[],
  failure: RunFailureProjection | null,
) {
  const nodes: Record<
    string,
    { total: number; valid: number; stale: number; cache: GraphCacheAppearance }
  > = {};
  const outputs: Record<string, GraphCacheAppearance> = {};
  const inputs: Record<string, GraphCacheAppearance> = {};
  const connections: Record<string, Record<string, ConnectionCacheState>> = {};
  for (const { output, state } of Object.values(cache?.outputs ?? {})) {
    const node = (nodes[output.port.nodeId] ??= { total: 0, valid: 0, stale: 0, cache: "new" });
    node.total++;
    if (state === "valid") node.valid++;
    if (state === "stale") node.stale++;
    outputs[portAddressKey(output.port)] = state === "missing" ? "new" : state;
  }
  for (const node of Object.values(nodes)) {
    node.cache =
      node.valid === node.total
        ? "valid"
        : node.valid > 0
          ? "partial"
          : node.stale > 0
            ? "stale"
            : "new";
  }
  for (const { output, input, state } of cache?.connections ?? []) {
    const key = portAddressKey(input);
    (connections[portAddressKey(output.port)] ??= {})[key] = state;
    const previous = inputs[key];
    inputs[key] =
      previous === "stale" || state === "stale"
        ? "stale"
        : previous === "new" || state === "new"
          ? "new"
          : "valid";
  }
  return {
    nodes,
    inputs,
    outputs,
    connections,
    compileStatus,
    runningNodes: new Set(runningNodeIds),
    failure,
  };
}

export type GraphResultPresentation = ReturnType<typeof projectGraphPresentation>;

export function graphElementState(
  presentation: GraphResultPresentation,
  nodeId: string,
  cache: GraphCacheAppearance = "new",
  diagnosticError = false,
): GraphElementState {
  if (diagnosticError || presentation.failure?.source?.nodeId === nodeId) return "error";
  if (presentation.compileStatus === "compiling") return "compiling";
  if (presentation.runningNodes.has(nodeId)) return "running";
  if (cache !== "new") return cache;
  return presentation.compileStatus === "compiled" ? "compiled" : "uncompiled";
}
