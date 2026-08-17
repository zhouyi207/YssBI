import { useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  editorDockviewPort,
  panelDockviewPort,
  hydrateDockviewLayout,
  persistDockviewLayoutDebounced,
  setDockviewLayoutWindowScope,
} from '@/features/core/dockview';
import { workbenchGridPort, useWorkbenchStore } from '@/features/core/workbench';
import { bootstrapEditorGraphSession } from '@/features/application/editor/bootstrapEditorGraphSession';

/** Restore and persist the outer workbench, shell Dockview, and nested editor layout. */
export function useWorkbenchLayout(): void {
  useEffect(() => {
    setDockviewLayoutWindowScope(getCurrentWindow().label);
    let disposed = false;
    void hydrateDockviewLayout().then(() => {
      if (disposed) return;
      const groupId = editorDockviewPort.getActiveGroupId();
      if (groupId) void bootstrapEditorGraphSession(groupId);
    });

    const persist = () => persistDockviewLayoutDebounced();
    const unsubscribeEditor = editorDockviewPort.subscribe(persist);
    const unsubscribePanel = panelDockviewPort.subscribe(persist);
    const unsubscribeWorkbench = workbenchGridPort.subscribe(persist);
    const unsubscribePreferences = useWorkbenchStore.subscribe(persist);
    return () => {
      disposed = true;
      unsubscribeEditor();
      unsubscribePanel();
      unsubscribeWorkbench();
      unsubscribePreferences();
    };
  }, []);
}
