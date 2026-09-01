/**
 * Stable editor group ids for shared session context.
 * Volatile tab/selection state lives in Dockview and pane state.
 */

import { useMemo } from "react";
import { useDockviewPortSnapshot } from "@/modules/workbench/public";
import { workbenchDockviewRead } from "@/modules/workbench/public";
import type { WorkbenchGroupInfo } from "@/modules/workbench/public";

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
