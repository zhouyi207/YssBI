import { useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { WindowStateService } from '@/services/window/windowStateService';
import { logger } from '@/utils/appLogger';
import { captureWindowGeometryPreservingMaximized } from './windowGeometryCapture';
import { readSecondaryWindowState, saveSecondaryWindowState } from './secondaryWindowGeometryStore';

/**
 * Editor workbench window geometry on close:
 * - `main` → backend `window_state.json` via `WindowStateService`
 * - secondary labels → per-label localStorage (`yssbi-secondary-window-*`)
 *
 * Mount-time geometry is applied at window creation (Rust setup / `createPersistedWindow`),
 * not here, to avoid resize flicker.
 */
export function useEditorWindowGeometryPersistence(): void {
  useEffect(() => {
    let cancelled = false;
    let unlistenClose: (() => void) | null = null;

    const setup = async () => {
      const win = getCurrentWindow();
      const isMain = win.label === 'main';

      try {
        const unlisten = await win.onCloseRequested(async () => {
          if (isMain) {
            const next = await captureWindowGeometryPreservingMaximized(
              win,
              () => WindowStateService.get('main'),
            );
            if (!next) return;
            try {
              await WindowStateService.save('main', next);
            } catch (error) {
              logger.app.error(
                `Failed to persist window state for main: ${error instanceof Error ? error.message : String(error)}`,
                'Window',
              );
            }
            return;
          }

          const next = await captureWindowGeometryPreservingMaximized(
            win,
            () => readSecondaryWindowState(win.label),
          );
          if (!next) return;
          saveSecondaryWindowState(win.label, next);
        });

        if (cancelled) {
          unlisten();
        } else {
          unlistenClose = unlisten;
        }
      } catch (error) {
        logger.app.warn(
          `Failed to attach editor window close listener: ${error instanceof Error ? error.message : String(error)}`,
          'Window',
        );
      }
    };

    void setup();
    return () => {
      cancelled = true;
      unlistenClose?.();
      unlistenClose = null;
    };
  }, []);
}
