import { ProjectService } from '@/services/project/projectService';
import { getViewport } from './viewportSession';

export function persistGraphViewport(graphPath: string | null | undefined): void {
  if (!graphPath) return;
  ProjectService.updateCanvas(graphPath, getViewport(graphPath)).catch(() => {});
}
