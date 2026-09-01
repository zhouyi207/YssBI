import type { useGraphManagement } from "@/features/application/dataManagement/useGraphManagement";
import type { useChartManagement } from "./useChartManagement";
import type { useEditorOperations } from "./useEditorOperations";
import type { useEditorPanelCommands } from "./useEditorPanelCommands";
import type { useGraphCanvasCommands } from "./useGraphCanvasCommands";
import type { useProjectOperations } from "./useProjectOperations";

export type WorkbenchCommandCapability = Pick<
  ReturnType<typeof useEditorOperations>,
  "undo" | "redo" | "copy" | "cut" | "paste" | "deleteSelected" | "duplicateSelected"
> &
  ReturnType<typeof useGraphCanvasCommands> &
  Pick<ReturnType<typeof useProjectOperations>, "saveGraph" | "saveGraphAs" | "importGraph"> &
  Pick<ReturnType<typeof useEditorPanelCommands>, "splitEditorRight"> &
  Pick<ReturnType<typeof useGraphManagement>, "addEvent" | "addFunction"> &
  Pick<ReturnType<typeof useChartManagement>, "addChart">;
