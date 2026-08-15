import { createContext, useContext, type ReactNode } from 'react';
import type { EditorSessionCommands } from './editorSessionCommands';
import { useEditorSessionCommands } from './useEditorSessionCommands';
import { useEditorSessionShared, type EditorSessionShared } from './useEditorSessionShared';

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

export type { EditorSession } from './editorSessionTypes';
