/**
 * 获取 events、functions、variables、dataframes 集合
 * Explorer 图列表来自 ResourceStore 快照；变量来自 variableStore。
 */

import { useMemo } from 'react';
import { useVariableStore, useDatabaseStore } from '@/features/core/dataStore';
import { useGraphResourcesByKind } from '@/features/core/resource';
import { useFunctionCatalog } from './useFunctionCatalog';
import type { EditorCollections } from '../editorCollections';

export function useEditorCollections(): EditorCollections {
  const events = useGraphResourcesByKind('event');
  const functions = useFunctionCatalog();
  const variables = useVariableStore((s) => s.variables);
  const dataframes = useDatabaseStore((s) => s.databases);

  return useMemo(
    () => ({ events, functions, variables, dataframes }),
    [events, functions, variables, dataframes]
  );
}
