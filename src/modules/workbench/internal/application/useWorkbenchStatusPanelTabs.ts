import { useEffect } from "react";

import { WORKBENCH_BOTTOM_DEFAULT_ORDER } from "../dockview/workbenchDockviewDefaults";
import { workbenchDockviewControl } from "../dockview/workbenchControl";
import { workbenchDockviewRead } from "../dockview/workbenchRead";
import { useDockviewPortSnapshot } from "../dockview/useDockviewPortSnapshot";
import { revealWorkbenchView } from "./workbenchLayoutActions";
import { showWorkbenchLayoutError } from "./workbenchLayoutErrorFeedback";

export function useWorkbenchStatusPanelTabs(position: "bottom" | "right") {
  const { hydrated } = useDockviewPortSnapshot(workbenchDockviewRead);
  const panels = workbenchDockviewRead.listPanels();
  const groups = workbenchDockviewRead.listGroups();
  const targetEdge = workbenchDockviewRead.getEdgeState(position);
  const groupPanels = targetEdge.groupId
    ? workbenchDockviewRead.listGroupPanels(targetEdge.groupId)
    : [];
  const defaultOrder =
    position === "bottom" ? WORKBENCH_BOTTOM_DEFAULT_ORDER : (["details", "assistant"] as const);
  const orderedViews = [...defaultOrder].sort((left, right) => {
    const indexOf = (viewId: string) => {
      const index = groupPanels.findIndex(
        (panel) => panel.metadata.role === "view" && panel.metadata.viewId === viewId,
      );
      return index < 0 ? Number.MAX_SAFE_INTEGER : index;
    };
    return indexOf(left) - indexOf(right);
  });

  useEffect(() => {
    // Restored layouts can still reserve the former collapsed tab strip.
    if (position === "bottom" && hydrated && targetEdge.collapsed && targetEdge.visible) {
      void workbenchDockviewControl
        .setEdgeCollapsed("bottom", true)
        .catch(showWorkbenchLayoutError);
    }
  }, [position, hydrated, targetEdge.collapsed, targetEdge.visible]);

  return orderedViews.map((viewId) => {
    const panel = panels.find(
      (candidate) => candidate.metadata.role === "view" && candidate.metadata.viewId === viewId,
    );
    const group = panel && groups.find((candidate) => candidate.groupId === panel.groupId);
    const edge =
      panel?.location.type === "edge"
        ? workbenchDockviewRead.getEdgeState(panel.location.position)
        : undefined;
    const selected = Boolean(
      panel &&
      group?.activePanelInstanceId === panel.panelInstanceId &&
      (!edge || (edge.visible && !edge.collapsed)),
    );

    return {
      viewId,
      selected,
      disabled: !hydrated,
      onSelect: () => {
        if (selected && panel?.location.type === "edge" && panel.location.position === position) {
          void workbenchDockviewControl
            .setEdgeCollapsed(position, true)
            .catch(showWorkbenchLayoutError);
        } else {
          void revealWorkbenchView(viewId);
        }
      },
    };
  });
}
