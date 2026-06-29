import { useEffect, useRef } from 'react';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useFirstGraphResource } from '@/features/core/resource';
import { useEditor } from './useEditor';

/**
 * 项目加载后若编辑器无打开 Tab，自动打开第一个图，避免「保存项目」等操作因无 activeTabId 不可用。
 */
export function useAutoOpenFirstGraph() {
  const { openGraph } = useEditor({ withCanvasInteraction: false });
  const currentPath = useProjectIOStore((s) => s.currentPath);
  const firstGraph = useFirstGraphResource();
  const openedForPathRef = useRef<string | null>(null);

  useEffect(() => {
    if (!currentPath || openedForPathRef.current === currentPath) return;

    const layout = useLayoutStore.getState();
    const editorGroupId = layout.activeEditorGroupId || 'default_editor';
    const tabs = layout.nodes[editorGroupId]?.data?.tabs ?? [];
    if (tabs.length > 0) {
      openedForPathRef.current = currentPath;
      return;
    }

    if (!firstGraph || (firstGraph.kind !== 'event' && firstGraph.kind !== 'function')) return;

    openedForPathRef.current = currentPath;
    void openGraph(firstGraph.id, firstGraph.name, firstGraph.kind);
  }, [currentPath, firstGraph, openGraph]);
}
