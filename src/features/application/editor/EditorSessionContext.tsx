import { createContext, useContext, useMemo, type ReactNode } from 'react';
import type { EditorSession } from './editorSessionTypes';
import type { EditorSessionCommands } from './editorSessionCommands';
import { useEditorSessionCommands } from './useEditorSessionCommands';
import { useEditorSessionShared, type EditorSessionShared } from './useEditorSessionShared';
import { useEditorGroupWorkspace } from '@/features/core/editor/hooks/useEditorGroupWorkspace';
import { useEditorSessionUi } from './useEditorSessionUi';
import { useEditorHistoryAvailability } from './useEditorHistoryAvailability';

const EditorSessionCommandsContext = createContext<EditorSessionCommands | null>(null);
const EditorSessionSharedContext = createContext<EditorSessionShared | null>(null);

export function EditorSessionProvider({ children }: { children: ReactNode }) {
  const commands = useEditorSessionCommands();
  const shared = useEditorSessionShared();

  return (
    <EditorSessionCommandsContext.Provider value={commands}>
      <EditorSessionSharedContext.Provider value={shared}>
        {children}
      </EditorSessionSharedContext.Provider>
    </EditorSessionCommandsContext.Provider>
  );
}

export function useEditorSessionCommandsContext(): EditorSessionCommands {
  const commands = useContext(EditorSessionCommandsContext);
  if (!commands) {
    throw new Error('useEditorSessionCommandsContext must be used within EditorSessionProvider');
  }
  return commands;
}

export function useEditorSessionSharedContext(): EditorSessionShared {
  const shared = useContext(EditorSessionSharedContext);
  if (!shared) {
    throw new Error('useEditorSessionSharedContext must be used within EditorSessionProvider');
  }
  return shared;
}

/**
 * Full session for focused editor group — use sparingly (sync, legacy callers).
 * Prefer useEditorSessionCommandsContext / useEditorSessionSharedContext / useEditorGroup.
 */
export function useEditorSession(): EditorSession {
  const commands = useEditorSessionCommandsContext();
  const shared = useEditorSessionSharedContext();
  const workspace = useEditorGroupWorkspace();
  const ui = useEditorSessionUi();
  const { canUndo, canRedo } = useEditorHistoryAvailability();

  return useMemo(
    (): EditorSession => ({
      ...shared,
      ...commands,
      ...ui,
      ...workspace,
      activeEditorGroupId: workspace.groupId,
      canUndo,
      canRedo,
    }),
    [commands, shared, workspace, ui, canUndo, canRedo],
  );
}

export type { EditorSession } from './editorSessionTypes';
