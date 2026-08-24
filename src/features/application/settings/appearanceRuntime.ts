import type { ThemeSettings } from '@/shared/types/settings';
import { resolveColorThemePreset } from './colorThemePresets';

/** Scroll area viewports only — not canvas pan/zoom or menubar chrome. */
export function applySmoothScrollSetting(enabled: boolean): void {
  document.documentElement.dataset.smoothScroll = enabled ? 'true' : 'false';
}

export function syncColorThemePreset(
  colorTheme: string,
  updateTheme: (updates: Partial<ThemeSettings>) => void,
): void {
  updateTheme(resolveColorThemePreset(colorTheme));
}
