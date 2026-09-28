import { useMemo } from "react";
import { useFileManagement } from "@/features/application/resource/useFileManagement";

import {
  useEditorOperations,
  useEditorPanelCommands,
  useGraphCanvasCommands,
  useProjectOperations,
  type WorkbenchCommandCapability,
} from "@/features/application/editor";

export function useWorkbenchCommandCoordinator(): WorkbenchCommandCapability {
  const editor = useEditorOperations();
  const canvas = useGraphCanvasCommands();
  const project = useProjectOperations();
  const panels = useEditorPanelCommands();
  const files = useFileManagement();

  return useMemo(
    () => ({
      undo: editor.undo,
      redo: editor.redo,
      copy: editor.copy,
      cut: editor.cut,
      paste: editor.paste,
      deleteSelected: editor.deleteSelected,
      duplicateSelected: editor.duplicateSelected,
      selectAllNodes: canvas.selectAllNodes,
      focusSelectedNodes: canvas.focusSelectedNodes,
      fitCompleteGraph: canvas.fitCompleteGraph,
      saveActiveFile: project.saveActiveFile,
      saveProjectAs: project.saveProjectAs,
      openProject: project.openProject,
      splitEditorRight: panels.splitEditorRight,
      createFile: files.createFile,
    }),
    [
      canvas.fitCompleteGraph,
      canvas.focusSelectedNodes,
      canvas.selectAllNodes,
      files.createFile,
      editor.copy,
      editor.cut,
      editor.deleteSelected,
      editor.duplicateSelected,
      editor.paste,
      editor.redo,
      editor.undo,
      panels.splitEditorRight,
      project.openProject,
      project.saveActiveFile,
      project.saveProjectAs,
    ],
  );
}
