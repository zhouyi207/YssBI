import { useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  editorDockviewPort,
  hydrateDockviewLayout,
  persistDockviewLayoutDebounced,
  setDockviewLayoutWindowScope,
} from '@/features/core/dockview';
import { workbenchGridPort, useWorkbenchStore } from '@/features/core/workbench';
import { bootstrapEditorGraphSession } from '@/features/application/editor/bootstrapEditorGraphSession';

/** Restore and persist the single Dockview-owned workbench/editor layout. */
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
    const unsubscribeWorkbench = workbenchGridPort.subscribe(persist);
    const unsubscribePreferences = useWorkbenchStore.subscribe(persist);
    return () => {
      disposed = true;
      unsubscribeEditor();
      unsubscribeWorkbench();
      unsubscribePreferences();
    };
  }, []);
}
