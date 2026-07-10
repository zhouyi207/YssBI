import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { readEditorViewStateViewport } from './editorViewStateMemento';

/** Resolve viewport on first open: project memento → default. */
export function resolveInitialGraphViewport(graphPath: string) {
  const projectPath = useProjectIOStore.getState().currentPath;
  if (projectPath) {
    const mementoViewport = readEditorViewStateViewport(projectPath, graphPath);
    if (mementoViewport) return mementoViewport;
  }
  return { ...DEFAULT_VIEWPORT };
}
