import { useGraphSessionUi } from "@/features/core/graphSession/ui";
import { workbenchDockviewRead } from "@/features/core/dockview";
import { resourceKey } from "@/features/core/resource";
import { useResourceRead } from "@/features/core/resource/read";
import type { ActiveProjectGraph } from "./projectResourceBrowser";

export function useActiveProjectGraph(): ActiveProjectGraph | null {
  const focusedSession = useGraphSessionUi((snapshot) => snapshot.focusedSession);
  const activeEditor = focusedSession
    ? (workbenchDockviewRead.getActiveEditorPanelInGroup(focusedSession.groupId)?.metadata ?? null)
    : null;

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
