/**
 * 编辑器组目录数据：variables、events、functions、dataframes、detailTarget
 * 直接使用 core hooks，无 application 依赖
 */
import { useMemo } from 'react';
import { useDetailTarget } from '../detail';
import { useEditorCollections } from './useEditorCollections';

export function useEditorGroupCatalog() {
  const collections = useEditorCollections();
  const detailTarget = useDetailTarget();

  return useMemo(
    () => ({
      ...collections,
      detailTarget,
    }),
    [collections, detailTarget],
  );
}
