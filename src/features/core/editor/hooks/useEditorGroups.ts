/**
 * Stable editor group ids for shared session context.
 * Volatile tab/selection state lives in Dockview and pane state.
 */

import { useMemo } from "react";
import { useDockviewPortSnapshot } from "@/features/core/dockview/useDockviewPortSnapshot";
import { workbenchDockviewRead } from "@/features/core/dockview/workbenchRead";
import type { WorkbenchGroupInfo } from "@/features/core/dockview/workbenchRead";

export function useEditorGroups(): WorkbenchGroupInfo[] {
  const { revision } = useDockviewPortSnapshot(workbenchDockviewRead);

  return useMemo(() => {
    const editorGroupIds = new Set(
      workbenchDockviewRead
        .listPanels()
        .filter((panel) => panel.metadata.role === "editor")
        .map((panel) => panel.groupId),
    );
    return workbenchDockviewRead.listGroups().filter((group) => editorGroupIds.has(group.groupId));
  }, [revision]);
}
