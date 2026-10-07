import type { useFileManagement } from "@/features/application/resource/useFileManagement";
import type { useEditorOperations } from "./useEditorOperations";
import type { useEditorPanelCommands } from "./useEditorPanelCommands";
import type { useGraphCanvasCommands } from "./useGraphCanvasCommands";
import type { useProjectOperations } from "./useProjectOperations";

export type WorkbenchCommandCapability = Pick<
  ReturnType<typeof useEditorOperations>,
  "undo" | "redo" | "copy" | "cut" | "paste" | "deleteSelected" | "duplicateSelected"
> &
  ReturnType<typeof useGraphCanvasCommands> &
  Pick<
    ReturnType<typeof useProjectOperations>,
    "saveActiveFile" | "saveProjectAs" | "openProject"
  > &
  Pick<ReturnType<typeof useEditorPanelCommands>, "splitEditorRight"> &
  ReturnType<typeof useFileManagement>;
