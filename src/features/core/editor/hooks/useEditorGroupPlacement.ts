import { useMemo } from "react";
import { useEditorPaneStateStore } from "@/features/core/dockview/editorPaneStateStore";
import { useDockviewPortSnapshot } from "@/features/core/dockview/useDockviewPortSnapshot";
import { workbenchDockviewRead } from "@/features/core/dockview/workbenchRead";
import type { WorkbenchEditorPanelInfo } from "@/features/core/dockview/workbenchRead";

export interface EditorGroupPlacementSlice {
  panelInstanceIds: string[];
  activeResourceRef: string | null;
  selectedNodeIds: string[];
  selectedConnectionIds: string[];
  panels: readonly WorkbenchEditorPanelInfo[];
}

/** Read-only projection of a Dockview group plus pane-local canvas selection. */
export function useEditorGroupPlacement(
  groupId: string | null | undefined,
): EditorGroupPlacementSlice {
  useDockviewPortSnapshot(workbenchDockviewRead);
  const group = groupId
    ? workbenchDockviewRead.listGroups().find((candidate) => candidate.groupId === groupId)
    : undefined;
  const panels = groupId ? workbenchDockviewRead.listEditorPanelsInGroup(groupId) : [];
  const activePanel = panels.find(
    (panel) => panel.panelInstanceId === group?.activePanelInstanceId,
  );
  const selection = useEditorPaneStateStore((state) =>
    activePanel ? state.selections[activePanel.panelInstanceId] : undefined,
  );

  return useMemo(() => {
    return {
      panelInstanceIds: panels.map((panel) => panel.panelInstanceId),
      activeResourceRef: activePanel?.metadata.resourceRef ?? null,
      selectedNodeIds: selection?.selectedNodeIds ?? [],
      selectedConnectionIds: selection?.selectedConnectionIds ?? [],
      panels,
    };
  }, [activePanel, panels, selection]);
}
