import { useRef } from "react";
import { useActiveEditorGroup, useEditorActions } from "@/features/core/editor";
import { useEditorOperations } from "./useEditorOperations";
import { useGraphCanvasCommands } from "./useGraphCanvasCommands";
import { useEditorPanelCommands } from "./useEditorPanelCommands";
import { useOpenChart, useChartManagement } from "./useChartManagement";
import { useProjectOperations } from "./useProjectOperations";
import {
  useGraphManagement,
  useVariableManagement,
  useDatabaseManagement,
  useNodeManagement,
} from "@/features/application/dataManagement";
import {
  pickEditorSessionLayoutBindings,
  pickEditorSessionNodeActions,
} from "./editorSessionTypes";
import {
  createEditorSessionCommandsContainer,
  patchEditorSessionCommands,
  type EditorSessionCommands,
} from "./editorSessionCommands";

/**
 * Builds the command surface once per provider mount.
 * The returned object identity is stable; fields are patched each render.
 */
export function useEditorSessionCommands(): EditorSessionCommands {
  const active = useActiveEditorGroup();
  const actions = useEditorActions(active);

  const editorOps = useEditorOperations();
  const canvasCommands = useGraphCanvasCommands();
  const panelCommands = useEditorPanelCommands();
  const openChart = useOpenChart();
  const chartMgmt = useChartManagement(openChart);
  const projectOps = useProjectOperations();

  const graphMgmt = useGraphManagement(panelCommands.openGraph);
  const variableMgmt = useVariableManagement();
  const dataFrameMgmt = useDatabaseManagement();
  const nodeMgmt = useNodeManagement();

  const layoutBindings = pickEditorSessionLayoutBindings(actions);
  const nodeActions = pickEditorSessionNodeActions(nodeMgmt);

  const containerRef = useRef<EditorSessionCommands | null>(null);
  if (!containerRef.current) {
    containerRef.current = createEditorSessionCommandsContainer();
  }

  return patchEditorSessionCommands(containerRef.current, {
    ...layoutBindings,
    ...editorOps,
    ...canvasCommands,
    ...panelCommands,
    openChart,
    ...chartMgmt,
    ...projectOps,
    ...graphMgmt,
    ...variableMgmt,
    ...dataFrameMgmt,
    ...nodeActions,
  });
}
