import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { patchEditorViewStateViewport } from './editorViewStateMemento';
import { getViewport } from './viewportSession';

/** Persist committed viewport to project-scoped editor view state (not graph files). */
export function persistGraphViewport(graphPath: string | null | undefined): void {
  if (!graphPath) return;
  const projectPath = useProjectIOStore.getState().currentPath;
  if (!projectPath) return;
  patchEditorViewStateViewport(projectPath, graphPath, getViewport(graphPath));
}
