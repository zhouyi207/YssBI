import type { DetailTarget } from '@/features/application/viewCapabilities';

/** Whether a sidebar row matches the current Detail panel focus. */
export function isSidebarDetailSelected(
  target: DetailTarget | null,
  kind: DetailTarget['kind'],
  resourceId: string,
): boolean {
  if (!target || target.kind !== kind) return false;
  if (target.kind === 'event' || target.kind === 'function') {
    return target.path === resourceId;
  }
  if ('id' in target) {
    return target.id === resourceId;
  }
  return false;
}
