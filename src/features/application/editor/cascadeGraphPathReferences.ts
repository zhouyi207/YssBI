import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { remapEditorViewStateGraphPath } from '@/features/core/viewport/editorViewStateMemento';

import { normalizeGraphResourcePath } from '@/shared/types/domain/graphResourcePath';

function pathsEqual(a: string, b: string): boolean {
  return normalizeGraphResourcePath(a) === normalizeGraphResourcePath(b);
}

function remapEditorGraphPaths(from: string, to: string): void {
  if (pathsEqual(from, to)) return;

  const store = useEditorStore.getState();
  const focus = store.detailFocus;

  if (focus?.kind === 'event' || focus?.kind === 'function') {
    if (focus.path === from) store.setDetailFocus({ ...focus, path: to });
  } else if (focus?.kind === 'node' && focus.graphPath === from) {
    store.setDetailFocus({ ...focus, graphPath: to });
  }

  if (store.variablesGraphScopePath === from) {
    store.setVariablesGraphScope(to);
  }
}

/** Migrate non-viewport editor UI state after the prepared viewport snapshot commits. */
export function remapGraphNonViewportUiState(from: string, to: string): void {
  if (pathsEqual(from, to)) return;
  remapEditorGraphPaths(from, to);
  const projectPath = useProjectIOStore.getState().currentPath;
  if (projectPath) remapEditorViewStateGraphPath(projectPath, from, to);
}
