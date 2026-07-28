import { invalidateGraphProjection } from '@/features/application/editorProjection/graphProjectionCoordinator';
import { commitFunctionSignature } from '@/features/application/editorMutation/functionSignatureCoordinator';
import { GraphService } from '@/services/graph/graphService';

import { markGraphTabDirty } from '@/features/core/layout/tabDirty';

export async function updateFunctionSignature(
  functionPath: string,
  patch: import('@/shared/types').FunctionSignaturePatch,
): Promise<void> {
  if (!patch.inputs && !patch.outputs) return;

  await commitFunctionSignature(functionPath, patch);
}

export async function updateCallFunctionTarget(
  graphPath: string,
  nodeId: string,
  functionPath: string,
): Promise<void> {
  await GraphService.updateCallFunctionTarget(graphPath, nodeId, functionPath);
  await invalidateGraphProjection(graphPath);
  markGraphTabDirty(graphPath);
}
