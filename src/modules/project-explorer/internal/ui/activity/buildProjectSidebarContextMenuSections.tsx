import type { TFunction } from "i18next";
import {
  VscAdd,
  VscChevronRight,
  VscCopy,
  VscEdit,
  VscFolderOpened,
  VscTrash,
} from "react-icons/vsc";

import type { ActionMenuSection } from "@/shared/ui/actionMenu";
import type {
  ProjectSidebarContextMenuActions,
  ProjectSidebarContextMenuState,
} from "./projectSidebarTypes";

export function buildProjectSidebarContextMenuSections(
  contextMenu: ProjectSidebarContextMenuState | null,
  actions: ProjectSidebarContextMenuActions,
  t: TFunction,
): ActionMenuSection[] {
  if (!contextMenu) return [];
  const target = contextMenu.target;

  if (target.type === "graph") {
    return [
      {
        items: [
          {
            id: "open",
            label: t("contextMenu.sidebar.open"),
            icon: <VscChevronRight size={12} />,
            onClick: () => actions.openGraph(target.id, target.name, target.graphType),
          },
          {
            id: "reveal-in-explorer",
            label: t("contextMenu.sidebar.revealInExplorer"),
            icon: <VscFolderOpened size={12} />,
            onClick: () => void actions.revealInExplorer({ kind: "graph", resourceId: target.id }),
          },
          {
            id: "rename",
            label: t("contextMenu.sidebar.rename"),
            icon: <VscEdit size={12} />,
            onClick: () => actions.renameGraphItem(target.id, target.name, target.graphType),
          },
          {
            id: "duplicate",
            label: t("contextMenu.sidebar.duplicate"),
            icon: <VscCopy size={12} />,
            onClick: () => void actions.duplicateGraphItem(target.id),
          },
        ],
      },
      {
        items: [
          {
            id: "delete",
            label: t("contextMenu.sidebar.delete"),
            icon: <VscTrash size={12} />,
            danger: true,
            onClick: () => void actions.deleteGraphItem(target.id, target.graphType),
          },
        ],
      },
    ];
  }

  if (target.type === "section") {
    return [
      {
        items: [
          {
            id: "new-graph",
            label:
              target.graphType === "event" ? t("canvas.newEventGraph") : t("canvas.newFunctionGraph"),
            icon: <VscAdd size={12} />,
            onClick: () => void actions.createGraph(target.graphType),
          },
        ],
      },
    ];
  }

  if (target.type === "chartSection") {
    return [
      {
        items: [
          {
            id: "new-chart",
            label: t("contextMenu.sidebar.newChart"),
            icon: <VscAdd size={12} />,
            onClick: () => void actions.addChart(),
          },
        ],
      },
    ];
  }

  if (target.type === "chart") {
    return [
      {
        items: [
          {
            id: "open",
            label: t("contextMenu.sidebar.open"),
            icon: <VscChevronRight size={12} />,
            onClick: () => actions.openChart(target.chartPath, target.name),
          },
          {
            id: "reveal-in-explorer",
            label: t("contextMenu.sidebar.revealInExplorer"),
            icon: <VscFolderOpened size={12} />,
            onClick: () =>
              void actions.revealInExplorer({ kind: "chart", resourceId: target.chartPath }),
          },
          {
            id: "rename",
            label: t("contextMenu.sidebar.rename"),
            icon: <VscEdit size={12} />,
            onClick: () => actions.renameChartItem(target.chartPath, target.name),
          },
          {
            id: "duplicate",
            label: t("contextMenu.sidebar.duplicate"),
            icon: <VscCopy size={12} />,
            onClick: () => void actions.duplicateChart(target.chartPath),
          },
        ],
      },
      {
        items: [
          {
            id: "delete",
            label: t("contextMenu.sidebar.delete"),
            icon: <VscTrash size={12} />,
            danger: true,
            onClick: () => void actions.deleteChart(target.chartPath),
          },
        ],
      },
    ];
  }

  return [];
}
