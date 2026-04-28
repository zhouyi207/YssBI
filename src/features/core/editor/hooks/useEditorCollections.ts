/**
 * 获取 events、functions、dataframes 集合
 * 依赖 dataStore
 */

import { useMemo } from 'react';
import { useGraphMetaStore, useVariableStore, useDatabaseStore } from '@/features/core/dataStore';

export function useEditorCollections() {
  const graphs = useGraphMetaStore((s) => s.graphs);
  const Variables = useVariableStore((s) => s.variables);
  const dataframes = useDatabaseStore((s) => s.databases);

  const events = useMemo(() => {
    const result: Record<string, any> = {};
    for (const [id, meta] of Object.entries(graphs)) {
      if (meta.type === 'event') result[id] = meta;
    }
    return result;
  }, [graphs]);

  const functions = useMemo(() => {
    const result: Record<string, any> = {};
    for (const [id, meta] of Object.entries(graphs)) {
      if (meta.type === 'function') result[id] = meta;
    }
    return result;
  }, [graphs]);

  return useMemo(
    () => ({ events, functions, Variables, dataframes }),
    [events, functions, Variables, dataframes]
  );
}
