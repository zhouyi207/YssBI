import { useGraphMetaStore } from '@/features/core/dataStore/graphMetaStore';
import type { FunctionSignaturePin, GraphType } from '@/shared/types';
import type { ProjectGraphIndexRow } from '@/services/project/projectService';

/** 从后端图 DTO / 领域图读取签名并写入 graphMetaStore（Detail 面板唯一来源）。 */
export type FunctionSignatureSource = {
  id: string;
  name: string;
  type: GraphType;
  functionInputs?: FunctionSignaturePin[];
  functionOutputs?: FunctionSignaturePin[];
};

export function syncFunctionSignatureFromGraph(graph: FunctionSignatureSource): void {
  if (graph.type !== 'function') return;

  const graphMetaStore = useGraphMetaStore.getState();
  const existing = graphMetaStore.graphs[graph.id];
  const signaturePatch = {
    functionInputs: graph.functionInputs ?? existing?.functionInputs ?? [],
    functionOutputs: graph.functionOutputs ?? existing?.functionOutputs ?? [],
  };

  if (existing) {
    graphMetaStore.updateGraph(graph.id, signaturePatch);
    return;
  }

  graphMetaStore.addGraph({
    id: graph.id,
    name: graph.name,
    type: graph.type,
    ...signaturePatch,
  });
}

/** 项目打开 / 索引刷新：从 `getProjectIndex` 的函数行 hydrate 签名表（与后端索引层对齐）。 */
export function hydrateFunctionSignaturesFromProjectIndex(
  graphs: ProjectGraphIndexRow[],
): void {
  const graphMetaStore = useGraphMetaStore.getState();
  for (const row of graphs) {
    if (row.type !== 'function') continue;
    const existing = graphMetaStore.graphs[row.id];
    const patch = {
      functionInputs: row.functionInputs ?? [],
      functionOutputs: row.functionOutputs ?? [],
    };
    if (existing) {
      graphMetaStore.updateGraph(row.id, patch);
      continue;
    }
    graphMetaStore.addGraph({
      id: row.id,
      name: row.name,
      type: 'function',
      ...patch,
    });
  }
}
