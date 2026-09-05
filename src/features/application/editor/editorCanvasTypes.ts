import type { useNodeManagement } from "@/features/application/dataManagement/useNodeManagement";
import type { CanvasMutationOutcome } from "@/features/core/canvas";
import type { EditorContextMenuState } from "@/features/core/editor";
import type { GraphSelection } from "@/modules/workbench/public";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import type { EditorCommandTarget } from "./editorCommandFocus";
import type { useEditorOperations } from "./useEditorOperations";
import type { useProjectOperations } from "./useProjectOperations";

export interface GraphCanvasViewportCommands {
  selectAllNodes(target: EditorCommandTarget): Promise<boolean>;
  focusSelectedNodes(target: EditorCommandTarget): boolean;
  fitCompleteGraph(target: EditorCommandTarget): boolean;
}

export type EditorCanvasMode = "interactive" | "preview";

export interface EditorCanvasScope {
  panelInstanceId: string;
  groupId: string;
  graphPath: string;
  graphKind: "event" | "function";
}

export type EditorCanvasCommandsSlice = Pick<
  ReturnType<typeof useEditorOperations>,
  | "copyNodes"
  | "cutNodes"
  | "duplicateNodes"
  | "deleteNodesById"
  | "breakAllNodeLinks"
  | "breakConnectionsById"
  | "selectLinkedNodes"
  | "disconnectPinById"
  | "resetPinValue"
  | "setSelectedNodeIds"
  | "setSelectedConnectionIds"
> &
  Pick<
    ReturnType<typeof useProjectOperations>,
    "compileGraph" | "executeGraph" | "cancelGraphExecution" | "clearGraphArtifacts"
  > &
  Pick<ReturnType<typeof useNodeManagement>, "createNode">;

export interface EditorCanvasWorkspaceSlice {
  groupId: string;
  activeGraph: {
    graphPath: string;
    kind: "event" | "function";
  } | null;
  selectedNodeIds: string[];
  selectedConnectionIds: string[];
  compileStatus: "uncompiled" | "compiling" | "compiled" | "blocked" | "failed";
}

export interface EditorCanvasInteractionSlice {
  contextMenu: EditorContextMenuState | null;
  setContextMenu: (menu: EditorContextMenuState | null) => void;
  pendingConnection: PinData | null;
  setPendingConnection: (pin: PinData | null) => void;
  onCanvasPointerDown: (event: React.PointerEvent) => void;
  onNodePointerDown: (nodeId: string, event: React.PointerEvent) => void;
  onPinPointerDown: (pin: PinData, event: React.PointerEvent) => void;
  insertRerouteAtConnection: (
    connectionId: string,
    position: Readonly<{ x: number; y: number }>,
    graphPath: string,
    groupId: string,
    selection: {
      before: GraphSelection;
      temporary: GraphSelection;
    },
  ) => Promise<CanvasMutationOutcome | false>;
}

export interface EditorCanvasSession {
  commands: EditorCanvasCommandsSlice;
  workspace: EditorCanvasWorkspaceSlice;
  interaction: EditorCanvasInteractionSlice;
}
