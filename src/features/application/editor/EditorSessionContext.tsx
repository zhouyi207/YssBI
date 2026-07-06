import { createContext, useContext, type ReactNode } from 'react';
import { useEditorSessionValue, type EditorSession } from './useEditorSessionValue';

const EditorSessionContext = createContext<EditorSession | null>(null);

export function EditorSessionProvider({ children }: { children: ReactNode }) {
  const session = useEditorSessionValue();
  return (
    <EditorSessionContext.Provider value={session}>
      {children}
    </EditorSessionContext.Provider>
  );
}

export function useEditorSession(): EditorSession {
  const session = useContext(EditorSessionContext);
  if (!session) {
    throw new Error('useEditorSession must be used within EditorSessionProvider');
  }
  return session;
}

export function useEditorSessionOptional(): EditorSession | null {
  return useContext(EditorSessionContext);
}

export type { EditorSession };
