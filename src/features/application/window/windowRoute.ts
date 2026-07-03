import type { WindowKind } from '@/shared/types/settings';

/** Map SPA hash route to persisted window kind. */
export function windowKindForRoute(route: string): WindowKind {
  switch (route) {
    case '/plot':
      return 'plot';
    case '/inspect':
      return 'sourceInspector';
    case '/info':
      return 'info';
    default:
      return 'info';
  }
}
