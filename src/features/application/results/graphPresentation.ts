import { shareProjection } from "@/features/core/state/readProjection";
import { portAddressKey } from "@/features/domain/editorProjection";
import type { RunFailureProjection } from "@/features/core/execution/executionTypes";
import type { GraphResultCacheProjection } from "./runtime";
import type { ConnectionCacheState } from "@/shared/types/domain/result";

export type GraphCacheAppearance = "new" | "stale" | "valid" | "partial";
export type GraphElementState = "unexecuted" | "running" | "error" | "valid" | "stale" | "partial";

/** Aggregate read-only facts per graph and retain unchanged element references. */
export function projectGraphPresentation(
  cache: GraphResultCacheProjection | undefined,
  runningNodeIds: readonly string[],
  failure: RunFailureProjection | null,
  previous?: GraphResultPresentation,
): GraphResultPresentation {
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
  const outputNodes = new Set(Object.keys(nodes));
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
    if (!outputNodes.has(input.nodeId)) {
      const node = (nodes[input.nodeId] ??= { total: 0, valid: 0, stale: 0, cache: "new" });
      node.total++;
      if (state === "valid") node.valid++;
      if (state === "stale") node.stale++;
    }
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
  const runningNodes = new Set(runningNodeIds);
  return shareProjection(previous, {
    nodes,
    inputs,
    outputs,
    connections,
    runningNodes:
      previous &&
      previous.runningNodes.size === runningNodes.size &&
      [...runningNodes].every((id) => previous.runningNodes.has(id))
        ? previous.runningNodes
        : runningNodes,
    failure,
  });
}

export interface GraphResultPresentation {
  nodes: Readonly<
    Record<string, { total: number; valid: number; stale: number; cache: GraphCacheAppearance }>
  >;
  inputs: Readonly<Record<string, GraphCacheAppearance>>;
  outputs: Readonly<Record<string, GraphCacheAppearance>>;
  connections: Readonly<Record<string, Readonly<Record<string, ConnectionCacheState>>>>;
  runningNodes: ReadonlySet<string>;
  failure: RunFailureProjection | null;
}

export function graphElementState(
  presentation: GraphResultPresentation,
  nodeId: string,
  cache: GraphCacheAppearance = "new",
  diagnosticError = false,
): GraphElementState {
  if (diagnosticError || presentation.failure?.source?.nodeId === nodeId) return "error";
  if (presentation.runningNodes.has(nodeId)) return "running";
  if (cache !== "new") return cache;
  return "unexecuted";
}
