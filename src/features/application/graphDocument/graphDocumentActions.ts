import { useGraphDataStore } from '@/features/core/dataStore';

import type { Graph } from '@/shared/types/domain';

import { GraphService } from '@/services/graph/graphService';

import {
  guardFullGraphPinRefresh,
  shouldSuppressIncrementalPinUpdate,
} from './incrementalPinUpdateGuard';

import { syncFunctionSignatureFromGraph } from './functionSignatureSync';



export async function updateFunctionSignature(

  functionId: string,

  patch: import('@/shared/types').FunctionSignaturePatch,

): Promise<{ sideEffectWarning: boolean }> {

  if (!patch.inputs && !patch.outputs) return { sideEffectWarning: false };



  const { graph, callerGraphs, sideEffectWarning } = await GraphService.updateFunctionSignature(

    functionId,

    patch,

  );

  const releaseGuard = guardFullGraphPinRefresh([

    functionId,

    ...callerGraphs.map((g) => g.id),

  ]);

  try {

    syncFunctionSignatureFromGraph(graph);

    useGraphDataStore.getState().addGraphFromData(functionId, graph);

    await applyCallerGraphUpdates(functionId, callerGraphs);

  } finally {

    releaseGuard();

  }

  return { sideEffectWarning };

}



const CALL_FUNCTION_NODE_TYPE = 'Functions:Call Function';



function graphHasCallToFunction(graphId: string, functionId: string): boolean {

  const bucket = useGraphDataStore.getState().graphEntities[graphId];

  if (!bucket) return false;

  return Object.values(bucket.nodes).some(

    (node) => node.nodeType === CALL_FUNCTION_NODE_TYPE && node.subGraphId === functionId,

  );

}



/**

 * 将 invoke 回包中已同步 Call pin 的调用方图写入前端 store（仅已加载的图）。

 * 若某已打开图含 Call 但不在回包中，再向后台 resolve 一次作为兜底。

 */

export async function applyCallerGraphUpdates(

  functionId: string,

  callerGraphs: Graph[],

): Promise<void> {

  const store = useGraphDataStore.getState();

  const appliedIds = new Set<string>();



  for (const graph of callerGraphs) {

    if (!store.hasGraph(graph.id)) continue;

    store.addGraphFromData(graph.id, graph);

    appliedIds.add(graph.id);

  }



  const fallbackIds = Object.keys(store.graphEntities).filter((graphId) => {

    if (graphId === functionId || appliedIds.has(graphId)) return false;

    return graphHasCallToFunction(graphId, functionId);

  });



  await Promise.all(

    fallbackIds.map(async (graphId) => {

      const graph = await GraphService.resolveGraphDynamicPins(graphId);

      useGraphDataStore.getState().addGraphFromData(graphId, graph);

    }),

  );

}



export { shouldSuppressIncrementalPinUpdate };
