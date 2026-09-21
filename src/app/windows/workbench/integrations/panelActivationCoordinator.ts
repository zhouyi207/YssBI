import { synchronizeActiveEditorPanel } from "@/features/application/editor/activateEditorPanelAndSyncSession";
import type { RootPanelActivationCoordinator } from "@/modules/workbench/public";

export const panelActivationCoordinator: RootPanelActivationCoordinator = (panel) => {
  synchronizeActiveEditorPanel(panel);
};
