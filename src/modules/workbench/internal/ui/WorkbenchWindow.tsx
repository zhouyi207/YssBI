import { useTranslation } from "react-i18next";
import type { FunctionComponent, ReactNode } from "react";

import { StatusBar } from "./status/StatusBar";
import { Menubar } from "./menu/Menubar";
import { RootDockviewHost, type RootDockviewHostProps } from "../dockview/RootDockviewHost";
import type { RootPanelRegistry, RootPanelTabComponent } from "../dockview/panelContribution";
import { WorkbenchOverlayHost } from "./overlay/WorkbenchOverlayHost";
import type { WorkbenchOverlayRegistry } from "./overlay/overlayContribution";
import { useAppInitialization } from "@/features/application/initialization";
import { LoadStatus } from "@/shared/types/ui";
import { useProjectSync } from "@/features/application/initialization";
import {
  useEditorKeyboard,
  useWorkbenchWindowCloseGuard,
  type WorkbenchCommandCapability,
} from "@/features/application/editor";

import { useWorkbenchWindowGeometryPersistence } from "@/features/application/window";
import { useProjectionLocaleSync } from "@/features/application/editor/useProjectionLocaleSync";

export interface WorkbenchWindowProps {
  readonly panelRegistry: RootPanelRegistry;
  readonly tabComponent: RootPanelTabComponent;
  readonly dndCoordinator: RootDockviewHostProps["dndCoordinator"];
  readonly onActiveEditorPanelChange: RootDockviewHostProps["onActiveEditorPanelChange"];
  readonly dockviewTheme: RootDockviewHostProps["dockviewTheme"];
  readonly commands: WorkbenchCommandCapability;
  readonly watermarkComponent: FunctionComponent;
  readonly dragOverlay?: ReactNode;
  readonly activityActions?: ReactNode;
  readonly overlays: WorkbenchOverlayRegistry;
}

function WorkbenchWindowReady({
  panelRegistry,
  tabComponent,
  dndCoordinator,
  onActiveEditorPanelChange,
  dockviewTheme,
  commands,
  watermarkComponent,
  dragOverlay,
  activityActions,
  overlays,
}: WorkbenchWindowProps) {
  useProjectSync();
  useProjectionLocaleSync();

  useEditorKeyboard(commands);

  return (
    <div
      className="flex h-screen w-full flex-col bg-[var(--workbench-bg)] text-foreground"
      data-yssbi-workbench
    >
      <Menubar commands={commands} />
      <div className="isolate flex min-h-0 flex-1 overflow-hidden">
        <RootDockviewHost
          panelRegistry={panelRegistry}
          tabComponent={tabComponent}
          dndCoordinator={dndCoordinator}
          onActiveEditorPanelChange={onActiveEditorPanelChange}
          dockviewTheme={dockviewTheme}
          watermarkComponent={watermarkComponent}
          dragOverlay={dragOverlay}
          activityActions={activityActions}
        />
      </div>
      <StatusBar />
      <WorkbenchOverlayHost overlays={overlays} />
    </div>
  );
}

export function WorkbenchWindow(props: WorkbenchWindowProps) {
  const { t } = useTranslation();
  const { status, error } = useAppInitialization();

  useWorkbenchWindowGeometryPersistence();
  useWorkbenchWindowCloseGuard();

  if (status !== LoadStatus.Ready) {
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

  return <WorkbenchWindowReady {...props} />;
}
