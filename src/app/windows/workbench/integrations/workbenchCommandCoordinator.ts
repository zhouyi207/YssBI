import { useMemo } from "react";

import { useGraphManagement } from "@/features/application/dataManagement";
import {
  useEditorOperations,
  useEditorPanelCommands,
  useGraphCanvasCommands,
  useProjectOperations,
  useChartManagement,
  useOpenChart,
  type WorkbenchCommandCapability,
} from "@/features/application/editor";

export function useWorkbenchCommandCoordinator(): WorkbenchCommandCapability {
  const editor = useEditorOperations();
  const canvas = useGraphCanvasCommands();
  const project = useProjectOperations();
  const panels = useEditorPanelCommands();
  const graphs = useGraphManagement(panels.openGraph);
  const openChart = useOpenChart();
  const charts = useChartManagement(openChart);

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
      saveGraph: project.saveGraph,
      saveGraphAs: project.saveGraphAs,
      importGraph: project.importGraph,
      splitEditorRight: panels.splitEditorRight,
      addEvent: graphs.addEvent,
      addFunction: graphs.addFunction,
      addChart: charts.addChart,
    }),
    [
      canvas.fitCompleteGraph,
      canvas.focusSelectedNodes,
      canvas.selectAllNodes,
      charts.addChart,
      editor.copy,
      editor.cut,
      editor.deleteSelected,
      editor.duplicateSelected,
      editor.paste,
      editor.redo,
      editor.undo,
      graphs.addEvent,
      graphs.addFunction,
      panels.splitEditorRight,
      project.importGraph,
      project.saveGraph,
      project.saveGraphAs,
    ],
  );
}
