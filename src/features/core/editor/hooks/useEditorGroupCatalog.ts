import { useMemo } from 'react';
import { useEditorGroup } from '@/features/application/editor/core/hooks/useEditorGroup';

export function useEditorGroupCatalog() {
  const {
    variables,
    Variables,
    events,
    functions,
    macros,
    dataframes,
    selectedItemId,
    selectedItemType,
  } = useEditorGroup();

  return useMemo(() => ({
    variables,
    Variables,
    events,
    functions,
    macros,
    dataframes,
    selectedItemId,
    selectedItemType,
  }), [
    variables,
    Variables,
    events,
    functions,
    macros,
    dataframes,
    selectedItemId,
    selectedItemType,
  ]);
}
