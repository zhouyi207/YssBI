import { ProjectService } from '@/services/project/projectService';
import { getViewport } from './viewportSession';

export function persistGraphViewport(graphId: string | null | undefined): void {
  if (!graphId) return;
  ProjectService.updateCanvas(graphId, getViewport(graphId)).catch(() => {});
}
