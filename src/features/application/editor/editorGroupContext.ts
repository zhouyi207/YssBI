import {
  useLayoutPortSnapshot,
  useEditorPaneStateStore,
  workbenchLayoutRead,
} from "@/modules/workbench/public";

/** Read the native central selection; hydration bookkeeping is not an active-tab fallback. */
export function getActiveGraphContext() {
  const panel = workbenchLayoutRead.getActiveEditorPanel();
  const kind = panel?.metadata.resourceKind;
  if (!panel || (kind !== "event" && kind !== "function")) return null;
  return { groupId: panel.groupId, graphPath: panel.metadata.resourceRef, kind };
}

export function useActiveGraphContext() {
  useLayoutPortSnapshot(workbenchLayoutRead);
  return getActiveGraphContext();
}

export function useActiveEditorGroup(overrideGroupId?: string | null) {
  useLayoutPortSnapshot(workbenchLayoutRead);
  const activeEditorGroupId = workbenchLayoutRead.getActiveEditorPanel()?.groupId ?? null;
  const groupId = overrideGroupId ?? activeEditorGroupId;
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
