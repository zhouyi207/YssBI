import { useMemo } from 'react';
import { useGraphMetaStore } from '@/features/core/dataStore/graphMetaStore';
import { useGraphResourcesByKind } from '@/features/core/resource/resourceSelectors';
import type { FunctionSignaturePin } from '@/shared/types';

export interface FunctionCatalogEntry {
  id: string;
  name: string;
  functionInputs: FunctionSignaturePin[];
  functionOutputs: FunctionSignaturePin[];
}

/** 函数 palette / 右键菜单：名称来自 ResourceStore，签名来自 graphMetaStore（项目索引 hydrate）。 */
export function useFunctionCatalog(): Record<string, FunctionCatalogEntry> {
  const resources = useGraphResourcesByKind('function');
  const metaGraphs = useGraphMetaStore((s) => s.graphs);

  return useMemo(() => {
    const result: Record<string, FunctionCatalogEntry> = {};
    for (const [id, resource] of Object.entries(resources)) {
      const meta = metaGraphs[id];
      result[id] = {
        id,
        name: resource.name,
        functionInputs: meta?.functionInputs ?? [],
        functionOutputs: meta?.functionOutputs ?? [],
      };
    }
    return result;
  }, [resources, metaGraphs]);
}
