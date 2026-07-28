import { useGraphMetaStore } from '@/features/core/dataStore/graphMetaStore';
import type { FunctionSignaturePin, GraphType } from '@/shared/types';
import { dataTypeFromDisplayString } from '@/shared/types/domain/dataType';
import {
  createDataSignaturePin,
} from '@/shared/types/domain/functionSignaturePin';
import type { FunctionSignatureDto } from '@/shared/types/dto/editorMutation';
import type { ProjectGraphIndexRow } from '@/services/project/projectService';

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

export function functionSignaturePins(signature: FunctionSignatureDto): {
  functionInputs: FunctionSignaturePin[];
  functionOutputs: FunctionSignaturePin[];
} {
  const functionInputs = signature.parameters.map((parameter) =>
    createDataSignaturePin(
      parameter.id,
      parameter.name,
      dataTypeFromDisplayString(parameter.type_name) ?? { kind: 'Any' },
    ),
  );
  const returnType = signature.return_type
    ? dataTypeFromDisplayString(signature.return_type) ?? { kind: 'Any' as const }
    : null;
  return {
    functionInputs,
    functionOutputs: returnType
      ? [createDataSignaturePin('return', 'Result', returnType)]
      : [],
  };
}



/** 项目打开 / 索引刷新：从 `getProjectIndex` 的函数行 hydrate 签名表（与后端索引层对齐）。 */
export function hydrateFunctionSignaturesFromProjectIndex(
  graphs: ProjectGraphIndexRow[],
): void {
  const graphMetaStore = useGraphMetaStore.getState();
  for (const row of graphs) {
    if (row.type !== 'function') continue;
    if (row.functionRevision == null || !row.functionSignature) continue;
    const existing = graphMetaStore.graphs[row.path];
    if (existing?.functionRevision != null
      && existing.functionRevision >= row.functionRevision) continue;
    const patch = {
      functionRevision: row.functionRevision,
      functionSignature: row.functionSignature,
      ...functionSignaturePins(row.functionSignature),
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
