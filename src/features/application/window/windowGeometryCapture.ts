import type { Window } from '@tauri-apps/api/window';
import type { WindowState } from '@/shared/types/settings';
import { logger } from '@/utils/appLogger';

export async function captureWindowGeometry(
  win: Window,
): Promise<WindowState | null> {
  try {
    const isMaximized = await win.isMaximized();
    if (isMaximized) {
      return null;
    }
    const size = await win.innerSize();
    const position = await win.outerPosition();
    return {
      width: size.width,
      height: size.height,
      x: position.x,
      y: position.y,
      isMaximized: false,
    };
  } catch (error) {
    logger.app.warn(
      `Failed to read current window geometry: ${error instanceof Error ? error.message : String(error)}`,
      'Window',
    );
    return null;
  }
}

export async function captureWindowGeometryPreservingMaximized(
  win: Window,
  readPrevious: () => WindowState | Promise<WindowState>,
): Promise<WindowState | null> {
  try {
    const isMaximized = await win.isMaximized();
    if (isMaximized) {
      const previous = await readPrevious();
      return { ...previous, isMaximized: true };
    }
    return captureWindowGeometry(win);
  } catch (error) {
    logger.app.warn(
      `Failed to read current window geometry: ${error instanceof Error ? error.message : String(error)}`,
      'Window',
    );
    return null;
  }
}
