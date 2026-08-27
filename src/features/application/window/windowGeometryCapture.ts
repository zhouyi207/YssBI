import type { WindowState } from '@/shared/types/settings';
import type { AppWindowHandle } from '@/services/platform/appWindow';

export async function captureWindowGeometry(
  win: Pick<AppWindowHandle, 'isMaximized' | 'innerSize' | 'outerPosition'>,
): Promise<WindowState | null> {
  try {
    const isMaximized = await win.isMaximized();
    if (!isMaximized.ok) return null;
    if (isMaximized.value) {
      return null;
    }
    const size = await win.innerSize();
    if (!size.ok) return null;
    const position = await win.outerPosition();
    if (!position.ok) return null;
    return {
      width: size.value.width,
      height: size.value.height,
      x: position.value.x,
      y: position.value.y,
      isMaximized: false,
    };
  } catch {
    return null;
  }
}

export async function captureWindowGeometryPreservingMaximized(
  win: Pick<AppWindowHandle, 'isMaximized' | 'innerSize' | 'outerPosition'>,
  readPrevious: () => WindowState | Promise<WindowState>,
): Promise<WindowState | null> {
  try {
    const isMaximized = await win.isMaximized();
    if (!isMaximized.ok) return null;
    if (isMaximized.value) {
      const previous = await readPrevious();
      return { ...previous, isMaximized: true };
    }
    return captureWindowGeometry(win);
  } catch {
    return null;
  }
}
