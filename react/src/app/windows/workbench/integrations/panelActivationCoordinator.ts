import { synchronizeActiveEditorPanel } from "@/features/application/editor/editorPanelActivation";
import type { RootPanelActivationCoordinator } from "@/modules/workbench/public";

export const panelActivationCoordinator: RootPanelActivationCoordinator = (panel) => {
  synchronizeActiveEditorPanel(panel);
};
