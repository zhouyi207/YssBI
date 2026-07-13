import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { patchEditorViewStateViewport } from './editorViewStateMemento';
import { getViewport } from './viewportSession';
import type { ViewportScope } from './viewportScope';

/** Persist the active pane viewport to project-scoped editor view state (per graph path). */
export function persistGraphViewport(scope: ViewportScope | null | undefined): void {
  if (!scope?.graphPath) return;
  const projectPath = useProjectIOStore.getState().currentPath;
  if (!projectPath) return;
  patchEditorViewStateViewport(projectPath, scope.graphPath, getViewport(scope));
}
