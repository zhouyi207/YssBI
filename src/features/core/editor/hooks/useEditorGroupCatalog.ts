/**
 * 编辑器组目录数据：variables、events、functions、dataframes、selectedItem
 * 直接使用 core hooks，无 application 依赖
 */
import { useMemo } from 'react';
import { useEditorCollections } from './useEditorCollections';
import { useEditorUIState } from './useEditorUIState';

export function useEditorGroupCatalog() {
  const collections = useEditorCollections();
  const { selectedItemId, selectedItemType } = useEditorUIState();

  return useMemo(
    () => ({
      ...collections,
      selectedItemId,
      selectedItemType,
    }),
    [collections, selectedItemId, selectedItemType]
  );
}
