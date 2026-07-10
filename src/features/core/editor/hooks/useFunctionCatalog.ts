import { useMemo } from 'react';
import { useGraphMetaStore } from '@/features/core/dataStore/graphMetaStore';
import { useGraphResourcesByKind } from '@/features/core/resource/resourceSelectors';
import {
  buildFunctionResourceCatalog,
  type FunctionResourceView,
} from '@/features/core/resource/functionResourceView';

/** 函数 palette / Detail / 右键菜单：名称 ResourceStore + 签名 graphMetaStore。 */
export function useFunctionCatalog(): Record<string, FunctionResourceView> {
  const resources = useGraphResourcesByKind('function');
  const metaGraphs = useGraphMetaStore((s) => s.graphs);

  return useMemo(
    () => buildFunctionResourceCatalog(resources, metaGraphs),
    [resources, metaGraphs],
  );
}
