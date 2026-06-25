import type { TFunction } from "i18next";
import { VscChevronRight, VscFolderOpened, VscStarEmpty, VscStarFull, VscTrash, VscWarning } from "react-icons/vsc";
import type { ContextMenuSection } from "@/shared/ui/contextMenu";
import type { ProjectPickerContextMenuActions, ProjectPickerContextMenuState } from "./projectPickerContextMenuTypes";

export function buildProjectPickerContextMenuSections(
  contextMenu: ProjectPickerContextMenuState | null,
  actions: ProjectPickerContextMenuActions,
  t: TFunction,
): ContextMenuSection[] {
  if (!contextMenu) return [];

  const project = contextMenu.target;
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
