import type { ManagedProject } from "@/features/application/project";
import type { PositionedContextMenuState } from "@/shared/ui/contextMenu";

export type ProjectPickerContextMenuState = PositionedContextMenuState<ManagedProject>;

export interface ProjectPickerContextMenuActions {
  openProject: (path: string) => void;
  toggleFavorite: (id: string) => void;
  removeProject: (id: string) => void;
  requestDeleteProjectFiles: (project: ManagedProject) => void;
  revealInExplorer: (path: string) => void;
  isBusy: boolean;
}
