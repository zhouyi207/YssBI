import { useCallback } from "react";
import { useTranslation } from "react-i18next";

import { updateVariableAction } from "@/features/application/dataManagement/variableActions";
import { useGraphManagement, useVariableManagement } from "@/features/application/dataManagement";
import { deleteChartWithConfirm } from "@/features/application/editor/chartDelete";
import {
  useChartManagement,
  useEditorPanelCommands,
  useOpenChart,
} from "@/features/application/editor";
import { renameResource } from "@/features/application/resource/resourceActions";
import { useActiveProjectGraph } from "@/features/application/sidebar";
import {
  renameChartResource,
  revealProjectResourceInExplorer,
} from "@/features/application/sidebar/sidebarResourceActions";
import { ui } from "@/features/core/ui/ui";
import { useVariableRead } from "@/features/core/variable/read";
import type { GraphResourceType } from "./projectSidebarTypes";

type OpenInputDialog = (
  title: string,
  value: string,
  onSubmit: (value: string) => void | Promise<void>,
  submitLabel?: string,
) => void;

export function useProjectActivityActions(openInputDialog: OpenInputDialog) {
  const { t } = useTranslation();
  const variables = useVariableRead((snapshot) => snapshot.variables);
  const activeProjectGraph = useActiveProjectGraph();
  const { openGraph } = useEditorPanelCommands();
  const {
    renameGraph,
    duplicateGraph,
    deleteEvent,
    deleteFunction,
    addEvent,
    addFunction,
    createGraph,
  } = useGraphManagement(openGraph);
  const { deleteVariable, addVariable } = useVariableManagement();
  const openChart = useOpenChart();
  const { duplicateChart, addChart } = useChartManagement(openChart);

  const renameGraphItem = useCallback(
    (id: string, name: string, type: GraphResourceType) => {
      openInputDialog(
        t("contextMenu.dialog.renameGraphTitle"),
        name,
        async (nextName) => {
          await renameGraph(id, nextName, type);
        },
        t("contextMenu.dialog.renameSubmit"),
      );
    },
    [openInputDialog, renameGraph, t],
  );

  const deleteGraphItem = useCallback(
    async (id: string, type: GraphResourceType) => {
      if (type === "event") {
        await deleteEvent(id);
        return;
      }
      await deleteFunction(id);
    },
    [deleteEvent, deleteFunction],
  );

  const duplicateGraphItem = useCallback(
    async (id: string) => {
      await duplicateGraph(id);
    },
    [duplicateGraph],
  );

  const renameVariableItem = useCallback(
    (id: string, name: string) => {
      openInputDialog(
        t("contextMenu.dialog.renameVariableTitle"),
        name,
        async (nextName) => {
          await renameResource({ id, kind: "variable" }, nextName);
        },
        t("contextMenu.dialog.renameSubmit"),
      );
    },
    [openInputDialog, t],
  );

  const deleteVariableItem = useCallback(
    async (id: string, name: string) => {
      const confirmed = await ui.confirm({
        title: t("sidebar.deleteVariableTitle"),
        message: t("sidebar.deleteVariableMessage", { name }),
        confirmText: t("contextMenu.sidebar.delete"),
        cancelText: t("common.cancel"),
        type: "danger",
      });
      if (!confirmed) return;
      await deleteVariable(id);
    },
    [deleteVariable, t],
  );

  const promoteVariable = useCallback(async (id: string) => {
    await updateVariableAction(id, { scope: { type: "global" } });
  }, []);

  const demoteVariable = useCallback(
    async (id: string) => {
      if (!activeProjectGraph) return;
      const scope =
        activeProjectGraph.kind === "function"
          ? { type: "function" as const, functionPath: activeProjectGraph.path }
          : { type: "event" as const, eventPath: activeProjectGraph.path };
      await updateVariableAction(id, { scope });
    },
    [activeProjectGraph],
  );

  const addProjectVariable = useCallback(
    async (name?: string, type: string = "Int64", isGlobal: boolean = false) => {
      if (!isGlobal && !activeProjectGraph) return null;
      return addVariable(
        name,
        type,
        isGlobal,
        activeProjectGraph
          ? {
              graphScope: {
                graphPath: activeProjectGraph.path,
                graphType: activeProjectGraph.kind,
              },
            }
          : undefined,
      );
    },
    [activeProjectGraph, addVariable],
  );

  const renameChartItem = useCallback(
    (chartPath: string, name: string) => {
      openInputDialog(
        t("contextMenu.dialog.renameChartTitle"),
        name,
        async (nextName) => {
          await renameChartResource(chartPath, nextName);
        },
        t("contextMenu.dialog.renameSubmit"),
      );
    },
    [openInputDialog, t],
  );

  const deleteChartItem = useCallback(async (chartPath: string) => {
    await deleteChartWithConfirm(chartPath);
  }, []);

  const revealInExplorer = useCallback(
    async (request: Parameters<typeof revealProjectResourceInExplorer>[0]) => {
      await revealProjectResourceInExplorer(request);
    },
    [],
  );

  const openVariableContextMenuTarget = useCallback(
    (id: string, name: string) => {
      const variable = variables[id];
      const isGlobal = variable?.scope.type === "global";
      return { type: "variable" as const, id, name, isGlobal };
    },
    [variables],
  );

  return {
    renameGraphItem,
    deleteGraphItem,
    duplicateGraphItem,
    renameVariableItem,
    deleteVariableItem,
    promoteVariable,
    demoteVariable,
    canDemoteVariable: activeProjectGraph !== null,
    renameChartItem,
    deleteChartItem,
    revealInExplorer,
    openVariableContextMenuTarget,
    addVariable: addProjectVariable,
    addEvent,
    addFunction,
    createGraph,
    openGraph,
    openChart,
    duplicateChart,
    addChart,
  };
}
