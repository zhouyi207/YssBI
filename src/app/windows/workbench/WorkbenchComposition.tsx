import { useCallback } from "react";

import { WatermarkView } from "@/modules/graph-editor/public";
import { NodeDocumentationModal } from "@/modules/node-catalog/public";
import { PluginActivityActions } from "@/modules/plugins/public";
import { SettingsView } from "@/modules/settings/public";
import { WorkbenchWindow, type WorkbenchOverlayRegistry } from "@/modules/workbench/public";
import { useApplicationThemeMode } from "@/features/application/settings/applicationSettings";
import { resolveYssbiDockviewTheme } from "@/shared/theme/dockviewTheme";
import { useActivityEditorDndCoordinator } from "./integrations/activityEditorDndCoordinator";
import { ActivityEditorDndOverlay } from "./integrations/activityEditorDndOverlay";
import { panelActivationCoordinator } from "./integrations/panelActivationCoordinator";
import { useWorkbenchCommandCoordinator } from "./integrations/workbenchCommandCoordinator";
import { rootPanelTabRenderer } from "./rootPanelTabRenderer";
import { rootPanelRegistry } from "./rootPanelRegistry";

const overlayRegistry = {
  settings: SettingsView,
  nodeDocumentation: NodeDocumentationModal,
} satisfies WorkbenchOverlayRegistry;

export function WorkbenchComposition() {
  const dndCoordinator = useActivityEditorDndCoordinator();
  const commands = useWorkbenchCommandCoordinator();
  const themeMode = useApplicationThemeMode();
  const watermarkComponent = useCallback(() => <WatermarkView commands={commands} />, [commands]);

  return (
    <WorkbenchWindow
      panelRegistry={rootPanelRegistry}
      tabComponent={rootPanelTabRenderer}
      dndCoordinator={dndCoordinator}
      onActiveEditorPanelChange={panelActivationCoordinator}
      dockviewTheme={resolveYssbiDockviewTheme(themeMode)}
      commands={commands}
      watermarkComponent={watermarkComponent}
      dragOverlay={<ActivityEditorDndOverlay />}
      activityActions={<PluginActivityActions />}
      overlays={overlayRegistry}
    />
  );
}
