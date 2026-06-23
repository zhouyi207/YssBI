import { useEffect, useRef } from 'react';
import { useGraphMetaStore } from '@/features/core/dataStore';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useEditor } from './useEditor';

/**
 * 项目加载后若编辑器无打开 Tab，自动打开第一个图，避免「保存项目」等操作因无 activeTabId 不可用。
 */
export function useAutoOpenFirstGraph() {
  const { openGraph } = useEditor({ withCanvasInteraction: false });
  const currentPath = useProjectIOStore((s) => s.currentPath);
  const graphOrder = useGraphMetaStore((s) => s.graphOrder);
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

    const firstId = graphOrder[0];
    const first = firstId ? useGraphMetaStore.getState().graphs[firstId] : null;
    if (!first) return;

    openedForPathRef.current = currentPath;
    void openGraph(first.id, first.name, first.type);
  }, [currentPath, graphOrder, openGraph]);
}
