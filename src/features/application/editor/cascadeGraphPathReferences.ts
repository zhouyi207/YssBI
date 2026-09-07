import { useEditorStore } from "@/features/core/editor/stores/useEditorStore";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { remapEditorViewStateGraphPath } from "@/features/core/viewport/editorViewStateMemento";

function remapEditorGraphPaths(from: string, to: string): void {
  if (from === to) return;

  const store = useEditorStore.getState();
  const focus = store.detailFocus;

  if (focus?.kind === "event" || focus?.kind === "function") {
    if (focus.path === from) store.setDetailFocus({ ...focus, path: to });
  } else if (focus?.kind === "node" && focus.graphPath === from) {
    store.setDetailFocus({ ...focus, graphPath: to });
  }

  if (store.variablesGraphScopePath === from) {
    store.setVariablesGraphScope(to);
  }
}

export function remapChartNonViewportUiState(from: string, to: string): void {
  if (from === to) return;
  const store = useEditorStore.getState();
  if (store.detailFocus?.kind === "chart" && store.detailFocus.chartPath === from) {
    store.setDetailFocus({ kind: "chart", chartPath: to });
  }
}

/** Migrate non-viewport editor UI state after the prepared viewport snapshot commits. */
export function remapGraphNonViewportUiState(from: string, to: string): void {
  if (from === to) return;
  remapEditorGraphPaths(from, to);
  const projectPath = useProjectIOStore.getState().currentPath;
  if (projectPath) remapEditorViewStateGraphPath(projectPath, from, to);
}
