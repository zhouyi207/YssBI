import {
  useDockviewPortSnapshot,
  useEditorPaneStateStore,
  workbenchDockviewRead,
} from "@/modules/workbench/public";

export function useActiveEditorGroup(overrideGroupId?: string | null) {
  useDockviewPortSnapshot(workbenchDockviewRead);
  const focusedEditorGroupId = workbenchDockviewRead.getActiveEditorPanel()?.groupId ?? null;
  const groupId = overrideGroupId ?? focusedEditorGroupId;
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

  return {
    activeResourceRef: activePanel?.metadata.resourceRef ?? null,
    panels,
    selectedNodeIds: selection?.selectedNodeIds ?? [],
    selectedConnectionIds: selection?.selectedConnectionIds ?? [],
  };
}
