import { useGraphDataStore } from '@/features/core/dataStore';

import type { Graph } from '@/shared/types/domain';

import { GraphService } from '@/services/graph/graphService';

import { CALL_FUNCTION_NODE_TYPE } from '@/features/domain/nodeDefinition';

import {
  guardFullGraphPinRefresh,
  shouldSuppressIncrementalPinUpdate,
} from './incrementalPinUpdateGuard';

import { syncFunctionSignatureFromGraph } from './functionSignatureSync';

export async function updateFunctionSignature(
  functionPath: string,
  patch: import('@/shared/types').FunctionSignaturePatch,
): Promise<{ sideEffectWarning: boolean }> {
  if (!patch.inputs && !patch.outputs) return { sideEffectWarning: false };

  const { graph, callerGraphs, sideEffectWarning } = await GraphService.updateFunctionSignature(
    functionPath,
    patch,
  );

  const releaseGuard = guardFullGraphPinRefresh([
    functionPath,
    ...callerGraphs.map((g) => g.path),
  ]);

  try {
    syncFunctionSignatureFromGraph(graph);
    useGraphDataStore.getState().addGraphFromData(functionPath, graph);
    await applyCallerGraphUpdates(functionPath, callerGraphs);
  } finally {
    releaseGuard();
  }

  return { sideEffectWarning };
}

function graphHasCallToFunction(callerGraphPath: string, functionPath: string): boolean {
  const bucket = useGraphDataStore.getState().graphEntities[callerGraphPath];
  if (!bucket) return false;

  return Object.values(bucket.nodes).some(
    (node) => node.nodeType === CALL_FUNCTION_NODE_TYPE && node.subGraphPath === functionPath,
  );
}

/** 将 invoke 回包中已同步 Call pin 的调用方图写入前端 store（仅已加载的图）。 */
export async function applyCallerGraphUpdates(
  functionPath: string,
  callerGraphs: Graph[],
): Promise<void> {
  const store = useGraphDataStore.getState();
  const appliedPaths = new Set<string>();

  for (const graph of callerGraphs) {
    if (!store.hasGraph(graph.path)) continue;
    store.addGraphFromData(graph.path, graph);
    appliedPaths.add(graph.path);
  }

  const fallbackPaths = Object.keys(store.graphEntities).filter((callerGraphPath) => {
    if (callerGraphPath === functionPath || appliedPaths.has(callerGraphPath)) return false;
    return graphHasCallToFunction(callerGraphPath, functionPath);
  });

  await Promise.all(
    fallbackPaths.map(async (callerGraphPath) => {
      const graph = await GraphService.resolveGraphDynamicPins(callerGraphPath);
      useGraphDataStore.getState().addGraphFromData(callerGraphPath, graph);
    }),
  );
}

export { shouldSuppressIncrementalPinUpdate };
