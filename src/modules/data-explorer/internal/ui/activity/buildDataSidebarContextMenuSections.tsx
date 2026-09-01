import type { TFunction } from "i18next";
import { VscAdd, VscChevronRight, VscEdit, VscFolderOpened, VscTrash } from "react-icons/vsc";

import type { ActionMenuSection } from "@/shared/ui/actionMenu";
import type {
  DataSidebarContextMenuActions,
  DataSidebarContextMenuState,
} from "./dataSidebarTypes";

export function buildDataSidebarContextMenuSections(
  contextMenu: DataSidebarContextMenuState | null,
  actions: DataSidebarContextMenuActions,
  t: TFunction,
): ActionMenuSection[] {
  if (!contextMenu) return [];
  const target = contextMenu.target;

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
