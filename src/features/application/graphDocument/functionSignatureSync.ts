import { useGraphMetaStore } from '@/features/core/dataStore/graphMetaStore';
import type { FunctionSignaturePin, GraphType } from '@/shared/types';
import type { FunctionSignatureDto } from '@/shared/types/domain/editorMutation';
import type { FunctionEditorProjectionDto } from '@/shared/types/domain/editorProjection';
import type { ProjectGraphIndexRow } from '@/shared/types/domain/project';
import type { PreparedFunctionDeltaInstall } from '@/features/application/editorMutation/projectPublicationCoordinator';

/** 从后端图 DTO / 领域图读取签名并写入 graphMetaStore（UI 签名唯一来源，见 functionResourceView）。 */
export type FunctionSignatureSource = {
  path: string;
  name: string;
  type: GraphType;
  functionInputs?: FunctionSignaturePin[];
  functionOutputs?: FunctionSignaturePin[];
};

export function syncFunctionSignatureFromGraph(graph: FunctionSignatureSource): void {
  if (graph.type !== 'function') return;

  const graphMetaStore = useGraphMetaStore.getState();
  const existing = graphMetaStore.graphs[graph.path];
  const signaturePatch = {
    functionInputs: graph.functionInputs ?? existing?.functionInputs ?? [],
    functionOutputs: graph.functionOutputs ?? existing?.functionOutputs ?? [],
  };

  if (existing) {
    graphMetaStore.updateGraph(graph.path, signaturePatch);
    return;
  }

  graphMetaStore.addGraph({
    path: graph.path,
    name: graph.name,
    type: graph.type,
    ...signaturePatch,
  });
}

export function installFunctionEditorProjection(
  graphPath: string,
  signature: FunctionSignatureDto,
  projection: FunctionEditorProjectionDto,
): PreparedFunctionDeltaInstall {
  return {
    graphPath,
    revision: projection.functionRevision,
    signature: structuredClone(signature),
    functionInputs: structuredClone(projection.inputs),
    functionOutputs: structuredClone(projection.outputs),
  };
}

function hasExactFunctionProjection(
  existing: ReturnType<typeof useGraphMetaStore.getState>['graphs'][string],
  signature: FunctionSignatureDto,
  projection: FunctionEditorProjectionDto,
): boolean {
  return JSON.stringify({
    signature: existing.functionSignature,
    inputs: existing.functionInputs,
    outputs: existing.functionOutputs,
  }) === JSON.stringify({
    signature,
    inputs: projection.inputs,
    outputs: projection.outputs,
  });
}

/** 项目打开 / 索引刷新：从 `getProjectIndex` 的函数行 hydrate 签名表（与后端索引层对齐）。 */
export function hydrateFunctionSignaturesFromProjectIndex(
  graphs: ProjectGraphIndexRow[],
): void {
  const graphMetaStore = useGraphMetaStore.getState();
  for (const row of graphs) {
    if (row.type !== 'function') continue;
    const existing = graphMetaStore.graphs[row.path];
    if (existing?.functionRevision != null) {
      if (existing.functionRevision > row.functionRevision) continue;
      if (existing.functionRevision === row.functionRevision
        && hasExactFunctionProjection(
          existing,
          row.functionSignature,
          row.functionEditorProjection,
        )) continue;
    }
    const install = installFunctionEditorProjection(
      row.path,
      row.functionSignature,
      row.functionEditorProjection,
    );
    const patch = {
      functionRevision: install.revision,
      functionSignature: install.signature,
      functionInputs: [...install.functionInputs],
      functionOutputs: [...install.functionOutputs],
    };
    if (existing) {
      graphMetaStore.updateGraph(row.path, patch);
      continue;
    }
    graphMetaStore.addGraph({
      path: row.path,
      name: row.name,
      type: 'function',
      ...patch,
    });
  }
}
