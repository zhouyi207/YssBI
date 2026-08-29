import { useEffect } from 'react';
import { currentAppWindow } from '@/services/platform/appWindow';
import { WindowStateService } from '@/services/window/windowStateService';
import { logger } from '@/features/application/observability/appLogger';
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
      const win = currentAppWindow();
      const isMain = win.label === 'main';

      try {
        const subscription = await win.onCloseRequested(async (): Promise<'allow'> => {
          if (isMain) {
            const next = await captureWindowGeometryPreservingMaximized(
              win,
              () => WindowStateService.get('main'),
            );
            if (!next) return 'allow';
            try {
              await WindowStateService.save('main', next);
            } catch {
              logger.app.error('window state persistence failed', 'Window');
            }
            return 'allow';
          }

          const next = await captureWindowGeometryPreservingMaximized(
            win,
            () => readSecondaryWindowState(win.label),
          );
          if (!next) return 'allow';
          saveSecondaryWindowState(win.label, next);
          return 'allow';
        });

        if (cancelled) {
          if (subscription.ok) subscription.value();
        } else if (subscription.ok) {
          unlistenClose = subscription.value;
        } else {
          logger.app.warn('editor window close listener unavailable', 'Window');
        }
      } catch {
        logger.app.warn('editor window close listener unavailable', 'Window');
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
