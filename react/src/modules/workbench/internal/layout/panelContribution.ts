import type { FunctionComponent } from "react";
import type { WorkbenchComponentId, WorkbenchPanelParams } from "./workbenchPanelModel";
import type { EditorPanelScope } from "./editorRenderer";

export interface RootPanelProps {
  readonly panelInstanceId: string;
  readonly groupId: string;
  readonly params: WorkbenchPanelParams;
  readonly title: string;
  readonly visible: boolean;
}
export type RootPanelComponent = FunctionComponent<RootPanelProps>;
export type RootPanelTabComponent = FunctionComponent<RootPanelProps>;
export interface RootPanelActivationTarget {
  readonly panelInstanceId: string;
  readonly groupId: string;
  readonly metadata: {
    readonly role: "editor";
    readonly resourceRef: string;
    readonly resourceKind: EditorPanelScope["resourceKind"];
  };
}
export type RootPanelRegistry = Record<WorkbenchComponentId, RootPanelComponent>;
