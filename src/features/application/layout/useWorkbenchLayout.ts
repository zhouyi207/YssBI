import { useEffect } from 'react';
import { hydrateWorkbenchLayout } from '@/features/core/layout/workbenchLayoutService';

/** Hydrate persisted workbench chrome + editor grid from localStorage on mount. */
export function useWorkbenchLayout(): void {
  useEffect(() => {
    hydrateWorkbenchLayout();
  }, []);
}
