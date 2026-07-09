import { useEffect } from 'react';
import {
  hydrateWorkbenchLayout,
  subscribeWorkbenchLayoutPersistence,
} from '@/features/core/layout/workbenchLayoutService';

/** Hydrate persisted workbench chrome sizes and subscribe to sash-end persistence. */
export function useWorkbenchLayout(): void {
  useEffect(() => {
    hydrateWorkbenchLayout();
    return subscribeWorkbenchLayoutPersistence();
  }, []);
}
