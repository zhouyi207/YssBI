import { produce } from "immer";
import { shallow } from "zustand/shallow";
import { freezePublishedValue } from "@/shared/types/deepReadonly";
import { portAddressKey } from "@/features/domain/editorProjection";
import type { RunFailureProjection } from "@/features/core/execution/executionTypes";
import type { GraphResultState } from "@/shared/types/domain/result";
import type { ConnectionCacheState } from "@/shared/types/domain/result";

export type GraphCacheAppearance = "new" | "stale" | "valid" | "partial";
export type GraphElementState = "unexecuted" | "running" | "error" | "valid" | "stale" | "partial";

const EMPTY_PENDING_OUTPUTS: ReadonlySet<string> = new Set();

/** Aggregate only result facts; running/failure changes do not invalidate these tables. */
export function projectGraphResultCache(
  cache: GraphResultState | undefined,
  pendingOutputs: ReadonlySet<string> = EMPTY_PENDING_OUTPUTS,
  previous?: GraphCachePresentation,
): GraphCachePresentation {
  const nodes: Record<
    string,
    { total: number; valid: number; stale: number; cache: GraphCacheAppearance }
  > = {};
  const outputs: Record<string, GraphCacheAppearance> = {};
  const inputs: Record<string, GraphCacheAppearance> = {};
  const connections: Record<string, Record<string, ConnectionCacheState>> = {};
  for (const { output, state: cachedState } of cache?.outputs ?? []) {
    const key = portAddressKey(output.port);
    const state = pendingOutputs.has(key) ? "missing" : cachedState;
    const node = (nodes[output.port.nodeId] ??= { total: 0, valid: 0, stale: 0, cache: "new" });
    node.total++;
    if (state === "valid") node.valid++;
    if (state === "stale") node.stale++;
    outputs[key] = state === "missing" ? "new" : state;
  }
  const outputNodes = new Set(Object.keys(nodes));
  for (const { output, input, state: cachedState } of cache?.connections ?? []) {
    const outputKey = portAddressKey(output.port);
    const state = pendingOutputs.has(outputKey) && cachedState === "valid" ? "stale" : cachedState;
    const key = portAddressKey(input);
    (connections[outputKey] ??= {})[key] = state;
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
  const result = produce<GraphCachePresentation>(
    previous ?? { nodes: {}, inputs: {}, outputs: {}, connections: {} },
    (draft) => {
      for (const id in nodes) {
        if (!shallow(previous?.nodes[id], nodes[id])) draft.nodes[id] = nodes[id];
      }
      for (const id in draft.nodes) if (!nodes[id]) delete draft.nodes[id];
      for (const id in inputs) draft.inputs[id] = inputs[id];
      for (const id in draft.inputs) if (!(id in inputs)) delete draft.inputs[id];
      for (const id in outputs) draft.outputs[id] = outputs[id];
      for (const id in draft.outputs) if (!(id in outputs)) delete draft.outputs[id];
      for (const id in connections) {
        if (!shallow(previous?.connections[id], connections[id]))
          draft.connections[id] = connections[id];
      }
      for (const id in draft.connections) if (!connections[id]) delete draft.connections[id];
    },
  );
  // These wide scalar dictionaries have no child objects. Publish them as roots so the
  // outer read projection can skip them when only execution appearance changes.
  freezePublishedValue(result.inputs);
  freezePublishedValue(result.outputs);
  return result;
}

/** Compose live execution appearance with the existing, independently derived result tables. */
export function projectGraphPresentation(
  cache: GraphCachePresentation,
  runningNodeIds: readonly string[],
  failure: RunFailureProjection | null,
  previous?: GraphResultPresentation,
): GraphResultPresentation {
  const runningNodes = new Set(runningNodeIds);
  const next = {
    ...cache,
    runningNodes:
      previous && shallow(previous.runningNodes, runningNodes)
        ? previous.runningNodes
        : runningNodes,
    failure,
  };
  return previous && shallow(previous, next) ? previous : next;
}

interface GraphCachePresentation {
  nodes: Readonly<
    Record<string, { total: number; valid: number; stale: number; cache: GraphCacheAppearance }>
  >;
  inputs: Readonly<Record<string, GraphCacheAppearance>>;
  outputs: Readonly<Record<string, GraphCacheAppearance>>;
  connections: Readonly<Record<string, Readonly<Record<string, ConnectionCacheState>>>>;
}

export interface GraphResultPresentation extends GraphCachePresentation {
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
