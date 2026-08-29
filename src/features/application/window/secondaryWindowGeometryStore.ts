import type { WindowState } from '@/shared/types/settings';
import { logger } from '@/features/application/observability/appLogger';

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

export function saveSecondaryWindowState(label: string, state: WindowState): void {
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

export function readSecondaryWindowFallbackPosition(label: string): { x: number; y: number } {
  const hash = label.split('').reduce((acc, ch) => acc + ch.charCodeAt(0), 0);
  return { x: 80 + (hash % 120), y: 80 + (hash % 80) };
}

export function readSecondaryWindowState(label: string): WindowState {
  const saved = loadSecondaryWindowState(label);
  if (saved) return saved;
  const { x, y } = readSecondaryWindowFallbackPosition(label);
  return {
    width: 1000,
    height: 700,
    x,
    y,
    isMaximized: false,
  };
}
