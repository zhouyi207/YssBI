import type { FunctionComponent, ReactNode } from "react";

import { RootLayoutHost, type RootLayoutHostProps } from "../layout/RootLayoutHost";
import type { RootPanelRegistry, RootPanelTabComponent } from "../layout/panelContribution";
import { WorkbenchOverlayHost } from "./overlay/WorkbenchOverlayHost";
import type { WorkbenchOverlayRegistry } from "./overlay/overlayContribution";

export interface WorkbenchWindowProps {
  readonly panelRegistry: RootPanelRegistry;
  readonly tabComponent: RootPanelTabComponent;
  readonly dndCoordinator: RootLayoutHostProps["dndCoordinator"];
  readonly onActiveEditorPanelChange: RootLayoutHostProps["onActiveEditorPanelChange"];
  readonly onClosePanel: RootLayoutHostProps["onClosePanel"];
  readonly onCloseGroup: RootLayoutHostProps["onCloseGroup"];
  readonly layoutTheme: RootLayoutHostProps["layoutTheme"];
  readonly watermarkComponent: FunctionComponent;
  readonly menuBar: ReactNode;
  readonly statusBar: ReactNode;
  readonly dragOverlay?: ReactNode;
  readonly overlays: WorkbenchOverlayRegistry;
}

export function WorkbenchWindow({
  panelRegistry,
  tabComponent,
  dndCoordinator,
  onActiveEditorPanelChange,
  layoutTheme,
  onClosePanel,
  onCloseGroup,
  watermarkComponent,
  menuBar,
  statusBar,
  dragOverlay,
  overlays,
}: WorkbenchWindowProps) {
  return (
    <div
      className="flex h-screen w-full flex-col bg-[var(--workbench-bg)] text-foreground"
      data-yssbi-workbench
    >
      {menuBar}
      <div className="isolate flex min-h-0 flex-1 overflow-hidden">
        <RootLayoutHost
          panelRegistry={panelRegistry}
          tabComponent={tabComponent}
          dndCoordinator={dndCoordinator}
          onActiveEditorPanelChange={onActiveEditorPanelChange}
          layoutTheme={layoutTheme}
          onClosePanel={onClosePanel}
          onCloseGroup={onCloseGroup}
          watermarkComponent={watermarkComponent}
          statusBar={statusBar}
          dragOverlay={dragOverlay}
        />
      </div>
      <WorkbenchOverlayHost overlays={overlays} />
    </div>
  );
}
