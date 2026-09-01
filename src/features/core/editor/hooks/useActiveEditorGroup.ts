import { workbenchDockviewRead } from "@/modules/workbench/public";
import { useDockviewPortSnapshot } from "@/modules/workbench/public";
import { createGraphSelection } from "../editorGroupSelection";
import { useEditorGroupPlacement } from "./useEditorGroupPlacement";

export function useActiveEditorGroup(overrideGroupId?: string | null) {
  useDockviewPortSnapshot(workbenchDockviewRead);
  const focusedEditorGroupId = workbenchDockviewRead.getActiveEditorPanel()?.groupId ?? null;
  const groupId = overrideGroupId ?? focusedEditorGroupId;
  const placement = useEditorGroupPlacement(groupId);

  return {
    groupId,
    focusedEditorGroupId,
    activeResourceRef: placement.activeResourceRef,
    panels: placement.panels,
    selectedNodeIds: placement.selectedNodeIds,
    selectedConnectionIds: placement.selectedConnectionIds,
    selection: createGraphSelection(placement.selectedNodeIds, placement.selectedConnectionIds),
    node: undefined,
  };
}
