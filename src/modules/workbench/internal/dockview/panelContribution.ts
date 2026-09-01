import type { FunctionComponent } from "react";
import type { IDockviewPanelProps } from "dockview-react";

import type { WorkbenchComponentId, WorkbenchPanelParams } from "./workbenchPanelModel";

export type RootDockviewPanelComponent = FunctionComponent<
  IDockviewPanelProps<WorkbenchPanelParams>
>;

export type RootPanelRegistry = Record<WorkbenchComponentId, RootDockviewPanelComponent>;
