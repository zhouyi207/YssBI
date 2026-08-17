import { createContext, useContext, type ReactNode } from 'react';
import type { EditorSessionCommands } from './editorSessionCommands';
import { useEditorSessionCommands } from './useEditorSessionCommands';

const EditorSessionCommandsContext = createContext<EditorSessionCommands | null>(null);

export function EditorSessionProvider({ children }: { children: ReactNode }) {
  const commands = useEditorSessionCommands();

  return (
    <EditorSessionCommandsContext.Provider value={commands}>
      {children}
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
