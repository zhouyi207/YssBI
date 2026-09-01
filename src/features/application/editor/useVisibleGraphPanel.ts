import { useEffect } from "react";
import {
  synchronizeVisibleGraphPanel,
  type VisibleGraphPanelScope,
} from "./synchronizeVisibleGraphPanel";

/** Synchronize only while Dockview keeps this panel in the visible layout. */
export function useVisibleGraphPanel(isVisible: boolean, scope: VisibleGraphPanelScope): void {
  useEffect(() => {
    if (!isVisible) return;
    void synchronizeVisibleGraphPanel({
      groupId: scope.groupId,
      graphPath: scope.graphPath,
    }).catch(() => undefined);
  }, [isVisible, scope.groupId, scope.graphPath]);
}
