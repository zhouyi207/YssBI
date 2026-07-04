import { useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { SourceService } from '@/services/resultSource/resultSourceService';
import { logger } from '@/utils/appLogger';

/** Release window-owned result sources when the Tauri window closes (not on React unmount). */
export function usePresentationWindowLifecycle(sourceId: string | null | undefined): void {
  useEffect(() => {
    if (!sourceId) return;

    const id = sourceId;
    let unlistenClose: (() => void) | null = null;

    const setup = async () => {
      try {
        unlistenClose = await getCurrentWindow().onCloseRequested(() => {
          void SourceService.releaseResultSource(id).catch((error) => {
            logger.app.warn(
              `releaseResultSource failed: ${error instanceof Error ? error.message : String(error)}`,
              'PresentationWindowLifecycle',
            );
          });
        });
      } catch (error) {
        logger.app.warn(
          `Failed to attach source release listener: ${error instanceof Error ? error.message : String(error)}`,
          'PresentationWindowLifecycle',
        );
      }
    };

    void setup();

    return () => {
      unlistenClose?.();
    };
  }, [sourceId]);
}
