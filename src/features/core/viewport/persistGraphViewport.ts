import { patchEditorViewStateViewport } from './editorViewStateMemento';
import { getViewport } from './viewportSession';
import { projectPathForViewport } from './projectPath';
import type { ViewportScope } from './viewportScope';

/** Persist the active pane viewport to project-scoped editor view state (per graph path). */
export function persistGraphViewport(scope: ViewportScope | null | undefined): void {
  if (!scope?.graphPath) return;
  const projectPath = projectPathForViewport();
  if (!projectPath) return;
  patchEditorViewStateViewport(projectPath, scope.graphPath, getViewport(scope));
}
