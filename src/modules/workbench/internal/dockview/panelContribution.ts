import type { FunctionComponent } from "react";
import type { IDockviewPanelHeaderProps, IDockviewPanelProps } from "dockview-react";

import type { WorkbenchComponentId, WorkbenchPanelParams } from "./workbenchPanelModel";
import type { EditorPanelScope } from "./editorRenderer";

export type RootDockviewPanelComponent = FunctionComponent<
  IDockviewPanelProps<WorkbenchPanelParams>
>;

export type RootPanelTabComponent = FunctionComponent<
  IDockviewPanelHeaderProps<WorkbenchPanelParams>
>;

export interface RootPanelActivationTarget {
  readonly panelInstanceId: string;
  readonly groupId: string;
  readonly metadata: {
    readonly role: "editor";
    readonly resourceRef: string;
    readonly resourceKind: EditorPanelScope["resourceKind"];
  };
}

export type RootPanelRegistry = Record<WorkbenchComponentId, RootDockviewPanelComponent>;
