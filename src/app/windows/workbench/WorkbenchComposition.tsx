import { useCallback } from "react";
import { useTranslation } from "react-i18next";

import { useEditorKeyboard, useWorkbenchWindowCloseGuard } from "@/features/application/editor";
import { useProjectionLocaleSync } from "@/features/application/editor/useProjectionLocaleSync";
import { useGraphProjectionSubscription } from "@/features/application/graphProjection";
import { useAppInitialization, useProjectSync } from "@/features/application/initialization";
import { useProjectProjection } from "@/features/application/project/projectProjection";
import { WatermarkView } from "@/modules/graph-editor/public";
import { NodeDocumentationModal } from "@/modules/node-catalog/public";
import { PluginActivityActions } from "@/modules/plugins/public";
import { SettingsView } from "@/modules/settings/public";
import { WorkbenchWindow, type WorkbenchOverlayRegistry } from "@/modules/workbench/public";
import { useApplicationThemeMode } from "@/features/application/settings/applicationSettings";
import { useWorkbenchWindowGeometryPersistence } from "@/features/application/window";
import { LoadStatus } from "@/shared/types/ui";
import { resolveYssbiDockviewTheme } from "@/shared/theme/dockviewTheme";
import { useActivityEditorDndCoordinator } from "./integrations/activityEditorDndCoordinator";
import { ActivityEditorDndOverlay } from "./integrations/activityEditorDndOverlay";
import { panelActivationCoordinator } from "./integrations/panelActivationCoordinator";
import { useWorkbenchCommandCoordinator } from "./integrations/workbenchCommandCoordinator";
import { WorkbenchMenuContribution } from "./menuContributionRegistry";
import { rootPanelTabRenderer } from "./rootPanelTabRenderer";
import { rootPanelRegistry } from "./rootPanelRegistry";
import { WorkbenchStatusBarContribution } from "./statusBarContributionRegistry";

const overlayRegistry = {
  settings: SettingsView,
  nodeDocumentation: NodeDocumentationModal,
} satisfies WorkbenchOverlayRegistry;

function WorkbenchReadyComposition() {
  const dndCoordinator = useActivityEditorDndCoordinator();
  const commands = useWorkbenchCommandCoordinator();
  const themeMode = useApplicationThemeMode();
  const { projectInstanceId } = useProjectProjection();
  const watermarkComponent = useCallback(() => <WatermarkView commands={commands} />, [commands]);

  useProjectSync();
  useGraphProjectionSubscription(projectInstanceId);
  useProjectionLocaleSync();
  useEditorKeyboard(commands);

  return (
    <WorkbenchWindow
      panelRegistry={rootPanelRegistry}
      tabComponent={rootPanelTabRenderer}
      dndCoordinator={dndCoordinator}
      onActiveEditorPanelChange={panelActivationCoordinator}
      dockviewTheme={resolveYssbiDockviewTheme(themeMode)}
      watermarkComponent={watermarkComponent}
      menuBar={<WorkbenchMenuContribution commands={commands} />}
      statusBar={<WorkbenchStatusBarContribution />}
      dragOverlay={<ActivityEditorDndOverlay />}
      activityActions={<PluginActivityActions />}
      overlays={overlayRegistry}
    />
  );
}

export function WorkbenchComposition() {
  const { t } = useTranslation();
  const { status, error } = useAppInitialization();

  useWorkbenchWindowGeometryPersistence();
  useWorkbenchWindowCloseGuard();

  if (status === LoadStatus.Ready) return <WorkbenchReadyComposition />;

  return (
    <div className="flex h-screen w-full items-center justify-center bg-[var(--workbench-bg)] text-sm text-muted-foreground">
      <div className="flex items-center gap-3 rounded-lg border border-[var(--strong-border)] bg-[var(--surface-raised)] px-4 py-3 shadow-lg">
        {!error ? (
          <span className="size-2 animate-pulse rounded-full bg-[var(--accent-color)]" />
        ) : null}
        {error ? t("editor.initializationFailed", { error }) : t("common.loading")}
      </div>
    </div>
  );
}
