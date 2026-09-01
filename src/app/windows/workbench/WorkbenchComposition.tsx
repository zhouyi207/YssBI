import { useCallback } from "react";

import { WorkbenchWindow } from "@/views/EditorView/WorkbenchWindow";
import { WatermarkView } from "@/views/EditorView/Canvas/overlays/WatermarkView";
import { useActivityEditorDndCoordinator } from "./integrations/activityEditorDndCoordinator";
import { useWorkbenchCommandCoordinator } from "./integrations/workbenchCommandCoordinator";
import { rootPanelRegistry } from "./rootPanelRegistry";

export function WorkbenchComposition() {
  const dndCoordinator = useActivityEditorDndCoordinator();
  const commands = useWorkbenchCommandCoordinator();
  const watermarkComponent = useCallback(() => <WatermarkView commands={commands} />, [commands]);

  return (
    <WorkbenchWindow
      panelRegistry={rootPanelRegistry}
      dndCoordinator={dndCoordinator}
      commands={commands}
      watermarkComponent={watermarkComponent}
    />
  );
}
