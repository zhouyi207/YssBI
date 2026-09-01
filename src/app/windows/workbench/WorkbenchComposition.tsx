import { useCallback } from "react";

import { WatermarkView } from "@/modules/graph-editor/public";
import { NodeDocumentationModal } from "@/modules/node-catalog/public";
import { PluginActivityActions } from "@/modules/plugins/public";
import { SettingsView } from "@/modules/settings/public";
import { WorkbenchWindow, type WorkbenchOverlayRegistry } from "@/modules/workbench/public";
import { useActivityEditorDndCoordinator } from "./integrations/activityEditorDndCoordinator";
import { useWorkbenchCommandCoordinator } from "./integrations/workbenchCommandCoordinator";
import { rootPanelRegistry } from "./rootPanelRegistry";

const overlayRegistry = {
  settings: SettingsView,
  nodeDocumentation: NodeDocumentationModal,
} satisfies WorkbenchOverlayRegistry;

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
      activityActions={<PluginActivityActions />}
      overlays={overlayRegistry}
    />
  );
}
