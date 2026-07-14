import { isSidebarSpawnDrag } from '@/features/core/dnd';
import { isSidebarItemDropAllowedAtPointer } from '@/features/core/layout/workbenchSidebarDropSurface';

/** Sidebar palette items may only drop on the editor workbench — not chrome (sidebar/detail/logs). */
export function isSidebarSpawnDropAllowed(
  data: unknown,
  pointer: { x: number; y: number } | null,
): boolean {
  if (!isSidebarSpawnDrag(data) || !pointer) return false;
  return isSidebarItemDropAllowedAtPointer(pointer.x, pointer.y);
}

export function isSidebarSpawnDropAllowedAtPointer(
  clientX: number,
  clientY: number,
): boolean {
  return isSidebarItemDropAllowedAtPointer(clientX, clientY);
}
