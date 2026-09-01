import { workbenchDockviewRead } from "@/modules/workbench/public";
import { useEditorStore } from "../stores/useEditorStore";

function readActiveGraphTab(): { id: string; type: "event" | "function" } | null {
  const panel = workbenchDockviewRead.getActiveEditorPanel();
  if (
    panel?.metadata.role === "editor" &&
    (panel.metadata.resourceKind === "event" || panel.metadata.resourceKind === "function")
  ) {
    return {
      id: panel.metadata.resourceRef,
      type: panel.metadata.resourceKind,
    };
  }
  return null;
}

export function syncVariablesGraphScopeFromActiveTab(): void {
  const activeGraph = readActiveGraphTab();
  if (activeGraph) {
    useEditorStore.getState().setVariablesGraphScope(activeGraph.id);
  }
}

export function syncVariablesGraphScopeAfterClose(closedGraphPath: string): void {
  const store = useEditorStore.getState();
  const activeGraph = readActiveGraphTab();

  if (activeGraph) {
    store.setVariablesGraphScope(activeGraph.id);
    return;
  }

  if (store.variablesGraphScopePath === closedGraphPath) {
    return;
  }
}

export function setVariablesGraphScopeFromResource(graphPath: string): void {
  useEditorStore.getState().setVariablesGraphScope(graphPath);
}
