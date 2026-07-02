import type { WindowKind } from '@/shared/types/settings';

/** Map SPA hash route to persisted window kind. */
export function windowKindForRoute(route: string): WindowKind {
  switch (route) {
    case '/plot':
      return 'plot';
    case '/view':
      return 'runtimeView';
    case '/info':
      return 'info';
    default:
      return 'info';
  }
}
