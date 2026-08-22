import type { TFunction } from "i18next";
import {
  VscChevronRight,
  VscClearAll,
  VscFolderOpened,
  VscNewFile,
  VscRefresh,
  VscStarEmpty,
  VscStarFull,
  VscTrash,
  VscWarning,
} from "react-icons/vsc";
import type { ManagedProject } from "@/features/application/project";
import type { ActionMenuSection } from "@/shared/ui/actionMenu";
import type { ProjectPickerContextMenuActions, ProjectPickerContextMenuState } from "./projectPickerContextMenuTypes";

function buildListContextMenuSections(
  actions: ProjectPickerContextMenuActions,
  t: TFunction,
): ActionMenuSection[] {
  return [
    {
      items: [
        {
          id: "new-project",
          label: t("projectPicker.newProject"),
          icon: <VscNewFile size={12} />,
          disabled: actions.isBusy,
          onClick: () => actions.newProject(),
        },
        {
          id: "import-project",
          label: t("projectPicker.importProject"),
          icon: <VscFolderOpened size={12} />,
          disabled: actions.isBusy,
          onClick: () => void actions.importProject(),
        },
        {
          id: "scan-projects",
          label: t("projectPicker.scanProjects"),
          icon: <VscRefresh size={12} />,
          disabled: actions.isBusy,
          onClick: () => void actions.scanProjects(),
        },
        {
          id: "cleanup-projects",
          label: t("projectPicker.cleanupProjects"),
          icon: <VscClearAll size={12} />,
          disabled: actions.isBusy,
          onClick: () => void actions.cleanupProjects(),
        },
      ],
    },
  ];
}

function buildProjectContextMenuSections(
  project: ManagedProject,
  actions: ProjectPickerContextMenuActions,
  t: TFunction,
): ActionMenuSection[] {
  const isFavorite = Boolean(project.isFavorite);

  return [
    {
      items: [
        {
          id: "open",
          label: t("projectPicker.enter"),
          icon: <VscChevronRight size={12} />,
          disabled: actions.isBusy,
          onClick: () => actions.openProject(project.path),
        },
        {
          id: "reveal-in-explorer",
          label: t("contextMenu.sidebar.revealInExplorer"),
          icon: <VscFolderOpened size={12} />,
          onClick: () => void actions.revealInExplorer(project.path),
        },
        {
          id: "toggle-favorite",
          label: isFavorite ? t("projectPicker.unfavorite") : t("projectPicker.favorite"),
          icon: isFavorite ? <VscStarFull size={12} /> : <VscStarEmpty size={12} />,
          onClick: () => actions.toggleFavorite(project.id),
        },
      ],
    },
    {
      items: [
        {
          id: "remove",
          label: t("projectPicker.removeFromList"),
          icon: <VscTrash size={12} />,
          danger: true,
          onClick: () => actions.removeProject(project.id),
        },
        {
          id: "delete-files",
          label: t("projectPicker.deleteProjectFiles"),
          icon: <VscWarning size={12} />,
          danger: true,
          onClick: () => actions.requestDeleteProjectFiles(project),
        },
      ],
    },
  ];
}

export function buildProjectPickerContextMenuSections(
  contextMenu: ProjectPickerContextMenuState | null,
  actions: ProjectPickerContextMenuActions,
  t: TFunction,
): ActionMenuSection[] {
  if (!contextMenu) return [];

  if (contextMenu.target.kind === "list") {
    return buildListContextMenuSections(actions, t);
  }

  return buildProjectContextMenuSections(contextMenu.target.project, actions, t);
}
