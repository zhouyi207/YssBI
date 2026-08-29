import { DEFAULT_VIEWPORT } from '@/shared/config-default';
import { readEditorViewStateViewport } from './editorViewStateMemento';
import { projectPathForViewport } from './projectPath';

/** Resolve viewport on first open: project memento → default. */
export function resolveInitialGraphViewport(graphPath: string) {
  const projectPath = projectPathForViewport();
  if (projectPath) {
    const mementoViewport = readEditorViewStateViewport(projectPath, graphPath);
    if (mementoViewport) return mementoViewport;
  }
  return { ...DEFAULT_VIEWPORT };
}
