import { useContext, useMemo } from "react";
import { workbenchDockviewRead } from "@/features/core/dockview/workbenchRead";
import { useDockviewPortSnapshot } from "@/features/core/dockview/useDockviewPortSnapshot";
import { createGraphSelection } from "../editorGroupSelection";
import { GroupContext } from "../context/GroupContext";
import { useEditorGroupPlacement } from "./useEditorGroupPlacement";

export function useEditorGroupWorkspace(overrideGroupId?: string | null) {
  const contextGroupId = useContext(GroupContext);
  useDockviewPortSnapshot(workbenchDockviewRead);
  const groupId =
    overrideGroupId ??
    contextGroupId ??
    workbenchDockviewRead.getActiveEditorPanel()?.groupId ??
    null;
  const placement = useEditorGroupPlacement(groupId);

  return useMemo(
    () => ({
      groupId,
      panels: placement.panels,
      activeTabId: placement.activeTabId,
      selectedNodeIds: placement.selectedNodeIds,
      selectedConnectionIds: placement.selectedConnectionIds,
      selection: createGraphSelection(placement.selectedNodeIds, placement.selectedConnectionIds),
    }),
    [groupId, placement],
  );
}
