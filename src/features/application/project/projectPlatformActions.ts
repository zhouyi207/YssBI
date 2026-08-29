import { ProjectService } from '@/services/project/projectService';
import {
  openPathDialog,
  type OpenPathDialogOptions,
} from '@/services/platform/pathDialog';

export function getDefaultProjectParentDirectory(): Promise<string> {
  return ProjectService.defaultProjectParentDirectory();
}

export function openProjectPathDialog(options: OpenPathDialogOptions) {
  return openPathDialog(options);
}
