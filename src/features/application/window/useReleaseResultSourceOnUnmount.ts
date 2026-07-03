import { useEffect } from 'react';
import { SourceService } from '@/services/resultSource/resultSourceService';
import { logger } from '@/utils/appLogger';

/** Release window-owned result source when a presentation window unmounts. */
export function useReleaseResultSourceOnUnmount(sourceId: string | null | undefined): void {
  useEffect(() => {
    if (!sourceId) return;
    const id = sourceId;
    return () => {
      void SourceService.releaseResultSource(id).catch((error) => {
        logger.app.warn(`releaseResultSource failed: ${String(error)}`, 'useReleaseResultSourceOnUnmount');
      });
    };
  }, [sourceId]);
}
