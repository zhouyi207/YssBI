import type { ManagedProject } from "@/features/application/project";
import type { PositionedActionMenuState } from "@/shared/ui/actionMenu";

export type ProjectPickerContextMenuTarget =
  | { kind: "project"; project: ManagedProject }
  | { kind: "list" };

export type ProjectPickerContextMenuState =
  PositionedActionMenuState<ProjectPickerContextMenuTarget>;

export interface ProjectPickerContextMenuActions {
  openProject: (path: string) => void;
  toggleFavorite: (id: string) => void;
  removeProject: (id: string) => void;
  requestDeleteProjectFiles: (project: ManagedProject) => void;
  revealInExplorer: (path: string) => void;
  newProject: () => void;
  importProject: () => void;
  scanProjects: () => void;
  cleanupProjects: () => void;
  isBusy: boolean;
}
