import { useCallback } from "react";
import { useTranslation } from "react-i18next";

import { useGraphManagement } from "@/features/application/dataManagement";
import { deleteChartWithConfirm } from "@/features/application/editor/chartDelete";
import {
  useChartManagement,
  useEditorPanelCommands,
  useOpenChart,
} from "@/features/application/editor";
import {
  renameChartResource,
  revealProjectResourceInExplorer,
} from "@/features/application/sidebar/sidebarResourceActions";
import type { GraphResourceType } from "./projectSidebarTypes";

type OpenInputDialog = (
  title: string,
  value: string,
  onSubmit: (value: string) => void | Promise<void>,
  submitLabel?: string,
) => void;

export function useProjectActivityActions(openInputDialog: OpenInputDialog) {
  const { t } = useTranslation();
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

  return {
    renameGraphItem,
    deleteGraphItem,
    duplicateGraphItem,
    renameChartItem,
    deleteChartItem,
    revealInExplorer,
    addEvent,
    addFunction,
    createGraph,
    openGraph,
    openChart,
    duplicateChart,
    addChart,
  };
}
