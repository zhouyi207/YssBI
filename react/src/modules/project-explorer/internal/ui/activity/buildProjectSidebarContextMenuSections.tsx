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

  if (target.type === "fileSection")
    return [
      {
        items: [
          {
            id: "new-file",
            label: t("documents.newFile"),
            icon: <VscAdd size={12} />,
            onClick: () => {
              actions.createFile(target.kind);
            },
          },
        ],
      },
    ];
  if (target.type === "file")
    return [
      {
        items: [
          {
            id: "open",
            label: t("contextMenu.sidebar.open"),
            icon: <VscChevronRight size={12} />,
            onClick: () => {
              actions.openFile(target);
            },
          },
          {
            id: "reveal-in-explorer",
            label: t("contextMenu.sidebar.revealInExplorer"),
            icon: <VscFolderOpened size={12} />,
            onClick: () => {
              actions.revealInExplorer({ kind: target.kind, resourceId: target.id });
            },
          },
          {
            id: "rename",
            label: t("contextMenu.sidebar.rename"),
            icon: <VscEdit size={12} />,
            onClick: () => {
              actions.renameFile(target, target.name);
            },
          },
          {
            id: "duplicate",
            label: t("contextMenu.sidebar.duplicate"),
            icon: <VscCopy size={12} />,
            onClick: () => {
              actions.duplicateFile(target);
            },
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
            onClick: () => {
              actions.deleteFile(target);
            },
          },
        ],
      },
    ];

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

  return [];
}
