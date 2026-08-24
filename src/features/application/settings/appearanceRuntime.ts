import type { ThemeSettings } from '@/shared/types/settings';
import { resolveColorThemePreset } from './colorThemePresets';

export type ActivityBarSide = 'left' | 'right';

export type ActivityBarLayout = {
  visible: boolean;
  side: ActivityBarSide;
};

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

export function resolveActivityBarLayout(
  activityBarPosition: string | undefined,
): ActivityBarLayout {
  if (activityBarPosition === 'Hidden') {
    return { visible: false, side: 'left' };
  }
  return {
    visible: true,
    side: activityBarPosition === 'Right' ? 'right' : 'left',
  };
}
