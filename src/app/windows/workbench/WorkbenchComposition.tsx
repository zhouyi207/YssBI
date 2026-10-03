import { requestCloseWorkbenchPanels } from "@/features/application/editor/workbenchPanelClose";
import { useTranslation } from "react-i18next";

import { useEditorKeyboard, useWorkbenchWindowCloseGuard } from "@/features/application/editor";
import { useProjectionLocaleSync } from "@/features/application/editor/useProjectionLocaleSync";
import { useAppInitialization, useProjectSync } from "@/features/application/initialization";
import { WatermarkView } from "@/modules/graph-editor/public";
import { WorkbenchWindow } from "@/modules/workbench/public";
import { useApplicationThemeMode } from "@/features/application/settings/applicationSettings";
import { LoadStatus } from "@/shared/types/ui";
import { resolveYssbiLayoutTheme } from "@/shared/theme/layoutTheme";
import { useActivityEditorDndCoordinator } from "./integrations/activityEditorDndCoordinator";
import { ActivityEditorDndOverlay } from "./integrations/activityEditorDndOverlay";
import { panelActivationCoordinator } from "./integrations/panelActivationCoordinator";
import { useWorkbenchCommandCoordinator } from "./integrations/workbenchCommandCoordinator";
import { WorkbenchMenuContribution } from "./menuContributionRegistry";
import { rootPanelTabRenderer } from "./rootPanelTabRenderer";
import { rootPanelRegistry } from "./rootPanelRegistry";
import { WorkbenchStatusBarContribution } from "./statusBarContributionRegistry";
import { PluginProvider } from "./integrations/PluginProvider";
import { useResultPanelLeases } from "@/features/application/results/useResultPanelLeases";
import { useUiIntents } from "@/features/application/presentation/useUiIntents";
import { openReferenceLink } from "@/features/application/editor/openReferenceLink";
import { MarkdownLinkContext } from "@/shared/ui/MarkdownLink";

const dragOverlay = <ActivityEditorDndOverlay />;
const statusBar = <WorkbenchStatusBarContribution />;
const closePanels = (ids: readonly string[]): void => {
  void requestCloseWorkbenchPanels(ids);
};

function WorkbenchReadyComposition() {
  const dndCoordinator = useActivityEditorDndCoordinator();
  const commands = useWorkbenchCommandCoordinator();
  const themeMode = useApplicationThemeMode();

  useProjectSync();
  useProjectionLocaleSync();
  useEditorKeyboard(commands);
  useResultPanelLeases();
  useUiIntents();

  return (
    <MarkdownLinkContext value={openReferenceLink}>
      <PluginProvider>
        <WorkbenchWindow
          panelRegistry={rootPanelRegistry}
          tabComponent={rootPanelTabRenderer}
          dndCoordinator={dndCoordinator}
          onActiveEditorPanelChange={panelActivationCoordinator}
          onClosePanels={closePanels}
          layoutTheme={resolveYssbiLayoutTheme(themeMode)}
          watermarkComponent={WatermarkView}
          menuBar={<WorkbenchMenuContribution commands={commands} />}
          statusBar={statusBar}
          dragOverlay={dragOverlay}
        />
      </PluginProvider>
    </MarkdownLinkContext>
  );
}

export function WorkbenchComposition() {
  const { t } = useTranslation();
  const { status, error } = useAppInitialization();

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
