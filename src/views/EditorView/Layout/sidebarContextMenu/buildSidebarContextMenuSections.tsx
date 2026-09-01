import type { TFunction } from "i18next";
import {
  VscAdd,
  VscChevronRight,
  VscCopy,
  VscEdit,
  VscFolderOpened,
  VscTrash,
} from "react-icons/vsc";
import { DEFAULT_VARIABLE_NAME } from "@/shared/constants/defaultResourceNames";
import type { ActionMenuSection } from "@/shared/ui/actionMenu";
import type { SidebarContextMenuActions, SidebarContextMenuState } from "./sidebarContextMenuTypes";

export function buildSidebarContextMenuSections(
  contextMenu: SidebarContextMenuState | null,
  actions: SidebarContextMenuActions,
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
              target.graphType === "event" ? t("canvas.newEventGraph") : t("canvas.newFunction"),
            icon: <VscAdd size={12} />,
            onClick: () => void actions.createGraph(target.graphType),
          },
        ],
      },
    ];
  }

  if (target.type === "variable") {
    const scopeItems = target.isGlobal
      ? [
          {
            id: "demote-to-local",
            label: t("sidebar.demoteToLocal"),
            icon: <VscEdit size={12} />,
            disabled: !actions.canDemoteVariable,
            title: actions.canDemoteVariable ? undefined : t("sidebar.noActiveGraph"),
            onClick: () => void actions.demoteVariable(target.id),
          },
        ]
      : [
          {
            id: "promote-to-global",
            label: t("sidebar.promoteToGlobal"),
            icon: <VscEdit size={12} />,
            onClick: () => void actions.promoteVariable(target.id),
          },
        ];

    return [
      {
        items: [
          {
            id: "rename",
            label: t("contextMenu.sidebar.rename"),
            icon: <VscEdit size={12} />,
            onClick: () => actions.renameVariableItem(target.id, target.name),
          },
          ...scopeItems,
        ],
      },
      {
        items: [
          {
            id: "delete",
            label: t("contextMenu.sidebar.delete"),
            icon: <VscTrash size={12} />,
            danger: true,
            onClick: () => void actions.deleteVariable(target.id, target.name),
          },
        ],
      },
    ];
  }

  if (target.type === "variableSection") {
    if (target.isGlobal === undefined) {
      return [
        {
          items: [
            {
              id: "new-local-variable",
              label: t("contextMenu.sidebar.newLocalVariable"),
              icon: <VscAdd size={12} />,
              onClick: () => void actions.addVariable(DEFAULT_VARIABLE_NAME, "Int64", false),
            },
            {
              id: "new-global-variable",
              label: t("contextMenu.sidebar.newGlobalVariable"),
              icon: <VscAdd size={12} />,
              onClick: () => void actions.addVariable(DEFAULT_VARIABLE_NAME, "Int64", true),
            },
          ],
        },
      ];
    }

    const isGlobal = target.isGlobal;
    return [
      {
        items: [
          {
            id: "new-variable",
            label: isGlobal
              ? t("contextMenu.sidebar.newGlobalVariable")
              : t("contextMenu.sidebar.newLocalVariable"),
            icon: <VscAdd size={12} />,
            onClick: () => void actions.addVariable(DEFAULT_VARIABLE_NAME, "Int64", isGlobal),
          },
        ],
      },
    ];
  }

  if (target.type === "database") {
    return [
      {
        items: [
          {
            id: "open",
            label: t("contextMenu.sidebar.open"),
            icon: <VscChevronRight size={12} />,
            onClick: () => actions.openDatabase(target.id),
          },
          {
            id: "view-editor",
            label: t("sidebar.viewInDatabaseEditor"),
            icon: <VscChevronRight size={12} />,
            onClick: () => actions.openDatabase(target.id),
          },
          {
            id: "reveal-in-explorer",
            label: t("contextMenu.sidebar.revealInExplorer"),
            icon: <VscFolderOpened size={12} />,
            onClick: () =>
              void actions.revealInExplorer({ kind: "database", resourceId: target.id }),
          },
          {
            id: "rename",
            label: t("contextMenu.sidebar.rename"),
            icon: <VscEdit size={12} />,
            onClick: () => actions.renameDatabaseItem(target.id, target.name),
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
            onClick: () => void actions.deleteDatabaseItem(target.id, target.name),
          },
        ],
      },
    ];
  }

  if (target.type === "dataSection") {
    return [
      {
        items: [
          {
            id: "import-data",
            label: t("contextMenu.sidebar.importData"),
            icon: <VscAdd size={12} />,
            onClick: () => actions.importData(),
          },
        ],
      },
    ];
  }

  if (target.type === "worksheetSection") {
    return [
      {
        items: [
          {
            id: "new-worksheet",
            label: t("contextMenu.sidebar.newWorksheet"),
            icon: <VscAdd size={12} />,
            onClick: () => void actions.addWorksheet(),
          },
        ],
      },
    ];
  }

  if (target.type === "worksheet") {
    return [
      {
        items: [
          {
            id: "open",
            label: t("contextMenu.sidebar.open"),
            icon: <VscChevronRight size={12} />,
            onClick: () => actions.openWorksheet(target.worksheetPath, target.name),
          },
          {
            id: "reveal-in-explorer",
            label: t("contextMenu.sidebar.revealInExplorer"),
            icon: <VscFolderOpened size={12} />,
            onClick: () =>
              void actions.revealInExplorer({
                kind: "worksheet",
                resourceId: target.worksheetPath,
              }),
          },
          {
            id: "rename",
            label: t("contextMenu.sidebar.rename"),
            icon: <VscEdit size={12} />,
            onClick: () => actions.renameWorksheetItem(target.worksheetPath, target.name),
          },
          {
            id: "duplicate",
            label: t("contextMenu.sidebar.duplicate"),
            icon: <VscCopy size={12} />,
            onClick: () => void actions.duplicateWorksheet(target.worksheetPath),
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
            onClick: () => void actions.deleteWorksheet(target.worksheetPath),
          },
        ],
      },
    ];
  }

  return [];
}
