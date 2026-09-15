import {
  useLayoutPortSnapshot,
  useEditorPaneStateStore,
  workbenchLayoutRead,
} from "@/modules/workbench/public";

export function useActiveEditorGroup(overrideGroupId?: string | null) {
  useLayoutPortSnapshot(workbenchLayoutRead);
  const focusedEditorGroupId = workbenchLayoutRead.getActiveEditorPanel()?.groupId ?? null;
  const groupId = overrideGroupId ?? focusedEditorGroupId;
  const group = groupId
    ? workbenchLayoutRead.listGroups().find((candidate) => candidate.groupId === groupId)
    : undefined;
  const panels = groupId ? workbenchLayoutRead.listEditorPanelsInGroup(groupId) : [];
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
