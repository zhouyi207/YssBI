import { useMemo } from 'react';
import { useEditorCollections, useEditorGroups } from '@/features/core/editor';
import type { EditorSessionResourcesSlice } from './editorSessionTypes';
import { pickEditorSessionResources } from './editorSessionTypes';
import type { EditorGroupSnapshot } from '@/shared/types';

export type EditorSessionShared = EditorSessionResourcesSlice & {
  groups: EditorGroupSnapshot[];
};

/**
 * Project collections + editor group list.
 * Does not include active-group workspace or transient canvas UI state.
 */
export function useEditorSessionShared(): EditorSessionShared {
  const collections = useEditorCollections();
  const groups = useEditorGroups();

  return useMemo(
    () => ({
      ...collections,
      groups,
    }),
    [collections, groups],
  );
}

export function pickSharedResources(shared: EditorSessionShared): EditorSessionResourcesSlice {
  return pickEditorSessionResources(shared as Parameters<typeof pickEditorSessionResources>[0]);
}
