import type { FunctionComponent, ReactNode } from "react";

import { RootLayoutHost, type RootLayoutHostProps } from "../layout/RootLayoutHost";
import type { RootPanelRegistry, RootPanelTabComponent } from "../layout/panelContribution";

export interface WorkbenchWindowProps {
  readonly panelRegistry: RootPanelRegistry;
  readonly tabComponent: RootPanelTabComponent;
  readonly dndCoordinator: RootLayoutHostProps["dndCoordinator"];
  readonly onActiveEditorPanelChange: RootLayoutHostProps["onActiveEditorPanelChange"];
  readonly onClosePanels: RootLayoutHostProps["onClosePanels"];
  readonly layoutTheme: RootLayoutHostProps["layoutTheme"];
  readonly watermarkComponent: FunctionComponent;
  readonly menuBar: ReactNode;
  readonly statusBar: ReactNode;
  readonly conversationToggle: ReactNode;
  readonly dragOverlay?: ReactNode;
}

export function WorkbenchWindow({
  panelRegistry,
  tabComponent,
  dndCoordinator,
  onActiveEditorPanelChange,
  layoutTheme,
  onClosePanels,
  watermarkComponent,
  menuBar,
  statusBar,
  conversationToggle,
  dragOverlay,
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
          onClosePanels={onClosePanels}
          watermarkComponent={watermarkComponent}
          statusBar={statusBar}
          conversationToggle={conversationToggle}
          dragOverlay={dragOverlay}
        />
      </div>
    </div>
  );
}
