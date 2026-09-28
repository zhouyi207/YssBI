import { useEditorStore } from "@/features/core/editor/stores/useEditorStore";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { remapEditorViewStateGraphPath } from "@/features/core/viewport/editorViewStateMemento";
import { remapDetailResource } from "@/features/core/editor/detail/editorDetailPolicy";

function remapEditorDetailResource(from: string, to: string): void {
  if (from === to) return;

  const store = useEditorStore.getState();
  const focus = store.detailFocus;

  const remapped = remapDetailResource(focus, from, to);
  if (remapped && remapped !== focus) store.setDetailFocus(remapped);
}

export function remapFileNonViewportUiState(from: string, to: string): void {
  remapEditorDetailResource(from, to);
}

/** Migrate non-viewport editor UI state after the prepared viewport snapshot commits. */
export function remapGraphNonViewportUiState(from: string, to: string): void {
  if (from === to) return;
  remapEditorDetailResource(from, to);
  const projectPath = useProjectIOStore.getState().currentPath;
  if (projectPath) remapEditorViewStateGraphPath(projectPath, from, to);
}
