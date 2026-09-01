import type { FunctionComponent, ReactNode } from "react";

import { RootDockviewHost, type RootDockviewHostProps } from "../dockview/RootDockviewHost";
import type { RootPanelRegistry, RootPanelTabComponent } from "../dockview/panelContribution";
import { WorkbenchOverlayHost } from "./overlay/WorkbenchOverlayHost";
import type { WorkbenchOverlayRegistry } from "./overlay/overlayContribution";

export interface WorkbenchWindowProps {
  readonly panelRegistry: RootPanelRegistry;
  readonly tabComponent: RootPanelTabComponent;
  readonly dndCoordinator: RootDockviewHostProps["dndCoordinator"];
  readonly onActiveEditorPanelChange: RootDockviewHostProps["onActiveEditorPanelChange"];
  readonly dockviewTheme: RootDockviewHostProps["dockviewTheme"];
  readonly watermarkComponent: FunctionComponent;
  readonly menuBar: ReactNode;
  readonly statusBar: ReactNode;
  readonly dragOverlay?: ReactNode;
  readonly activityActions?: ReactNode;
  readonly overlays: WorkbenchOverlayRegistry;
}

export function WorkbenchWindow({
  panelRegistry,
  tabComponent,
  dndCoordinator,
  onActiveEditorPanelChange,
  dockviewTheme,
  watermarkComponent,
  menuBar,
  statusBar,
  dragOverlay,
  activityActions,
  overlays,
}: WorkbenchWindowProps) {
  return (
    <div
      className="flex h-screen w-full flex-col bg-[var(--workbench-bg)] text-foreground"
      data-yssbi-workbench
    >
      {menuBar}
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
      {statusBar}
      <WorkbenchOverlayHost overlays={overlays} />
    </div>
  );
}
