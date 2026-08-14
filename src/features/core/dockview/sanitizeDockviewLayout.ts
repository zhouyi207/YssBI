import type { DockviewLayout } from './types';

/** Keep restored editor layouts inside the coordinated, in-window topology. */
export function sanitizeDockviewLayout(layout: DockviewLayout): DockviewLayout {
  const sanitized = { ...layout };
  delete sanitized.floatingGroups;
  delete sanitized.popoutGroups;
  return sanitized;
}
