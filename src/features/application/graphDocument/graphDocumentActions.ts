import { syncFunctionSignatureMeta, useGraphDataStore } from '@/features/core/dataStore';
import type { FunctionSignaturePatch } from '@/shared/types';
import { GraphService } from '@/services/graph/graphService';

export async function updateFunctionSignature(
  functionId: string,
  patch: FunctionSignaturePatch,
): Promise<void> {
  if (!patch.inputs && !patch.outputs) return;

  const updatedGraph = await GraphService.updateFunctionSignature(functionId, patch);
  syncFunctionSignatureMeta(updatedGraph);
  useGraphDataStore.getState().addGraphFromData(functionId, updatedGraph);
}
