import type { FunctionSignaturePin, Graph } from '@/shared/types';
import { useGraphMetaStore } from './graphMetaStore';

type FunctionSignatureMetaInput = Pick<Graph, 'id' | 'name' | 'type'> & {
  functionInputs?: FunctionSignaturePin[];
  functionOutputs?: FunctionSignaturePin[];
  folderPath?: string;
};

export function syncFunctionSignatureMeta(graph: FunctionSignatureMetaInput): void {
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
    folderPath: graph.folderPath,
    ...signaturePatch,
  });
}
