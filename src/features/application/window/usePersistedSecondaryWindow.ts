import { PhysicalPosition, PhysicalSize } from '@tauri-apps/api/dpi';
import { useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { WindowState } from '@/shared/types/settings';
import { logger } from '@/utils/appLogger';

const SECONDARY_WINDOW_PREFIX = 'yssbi-secondary-window-';

function storageKey(label: string): string {
  return `${SECONDARY_WINDOW_PREFIX}${label}`;
}

function loadSecondaryWindowState(label: string): WindowState | null {
  if (typeof localStorage === 'undefined') return null;
  try {
    const raw = localStorage.getItem(storageKey(label));
    if (!raw) return null;
    return JSON.parse(raw) as WindowState;
  } catch {
    return null;
  }
}

function saveSecondaryWindowState(label: string, state: WindowState): void {
  if (typeof localStorage === 'undefined') return;
  try {
    localStorage.setItem(storageKey(label), JSON.stringify(state));
  } catch (error) {
    logger.app.warn(
      `Failed to save secondary window state: ${error instanceof Error ? error.message : String(error)}`,
      'Window',
    );
  }
}

/** Persist geometry for auxiliary editor windows (label !== "main") in localStorage. */
export function usePersistedSecondaryWindow(): void {
  useEffect(() => {
    let cancelled = false;

    const setup = async () => {
      const win = getCurrentWindow();
      if (win.label === 'main') return;

      const saved = loadSecondaryWindowState(win.label);
      if (saved && !saved.isMaximized) {
        try {
          if (typeof saved.x === 'number' && typeof saved.y === 'number') {
            await win.setPosition(new PhysicalPosition(saved.x, saved.y));
          }
          await win.setSize(new PhysicalSize(saved.width, saved.height));
        } catch {
          // ignore apply failures on secondary windows
        }
      }

      const unlisten = await win.onCloseRequested(async () => {
        try {
          const isMaximized = await win.isMaximized();
          if (isMaximized) {
            const prev = loadSecondaryWindowState(win.label);
            if (prev) saveSecondaryWindowState(win.label, { ...prev, isMaximized: true });
            return;
          }
          const size = await win.innerSize();
          const position = await win.outerPosition();
          saveSecondaryWindowState(win.label, {
            width: size.width,
            height: size.height,
            x: position.x,
            y: position.y,
            isMaximized: false,
          });
        } catch (error) {
          logger.app.warn(
            `Failed to persist secondary window: ${error instanceof Error ? error.message : String(error)}`,
            'Window',
          );
        }
      });

      if (cancelled) {
        unlisten();
      }
    };

    void setup();
    return () => {
      cancelled = true;
    };
  }, []);
}

export function readSecondaryWindowFallbackPosition(label: string): { x: number; y: number } {
  const hash = label.split('').reduce((acc, ch) => acc + ch.charCodeAt(0), 0);
  return { x: 80 + (hash % 120), y: 80 + (hash % 80) };
}
