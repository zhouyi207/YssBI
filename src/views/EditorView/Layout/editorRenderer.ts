import type { ComponentType } from "react";

import type { EditorResourceKind } from "@/features/core/dockview";

export interface EditorPanelScope<Kind extends EditorResourceKind = EditorResourceKind> {
  readonly panelInstanceId: string;
  readonly groupId: string;
  readonly resourceRef: string;
  readonly resourceKind: Kind;
  readonly isVisible: boolean;
}

export type EditorRendererRegistry = {
  readonly [Kind in EditorResourceKind]: ComponentType<EditorPanelScope<Kind>>;
};
