import { useGraphSessionUi } from "@/features/core/graphSession/ui";
import { useLayoutPortSnapshot, workbenchLayoutRead } from "@/modules/workbench/public";
import { resourceKey } from "@/features/core/resource";
import { useResourceRead } from "@/features/core/resource/read";
export interface ActiveProjectGraph {
  path: string;
  kind: "event" | "function";
  name: string;
}

export function useActiveProjectGraph(): ActiveProjectGraph | null {
  useLayoutPortSnapshot(workbenchLayoutRead);
  const focusedSession = useGraphSessionUi((snapshot) => snapshot.focusedSession);
  // Tool focus keeps the existing graph context; an actual editor switch always takes priority.
  const activeEditor =
    workbenchLayoutRead.getActiveEditorPanel()?.metadata ??
    (focusedSession
      ? workbenchLayoutRead.getActiveEditorPanelInGroup(focusedSession.groupId)?.metadata
      : null);

  return useResourceRead((snapshot) => {
    if (
      !activeEditor ||
      activeEditor.role !== "editor" ||
      (activeEditor.resourceKind !== "event" && activeEditor.resourceKind !== "function")
    ) {
      return null;
    }
    const resource =
      snapshot.resources[
        resourceKey({ id: activeEditor.resourceRef, kind: activeEditor.resourceKind })
      ];
    return resource
      ? {
          path: activeEditor.resourceRef,
          kind: activeEditor.resourceKind,
          name: resource.name,
        }
      : null;
  });
}
