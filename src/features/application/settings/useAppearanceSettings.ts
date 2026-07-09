import { useEffect } from 'react';
import { useSettingsStore } from '@/features/core/settings/settingsStore';
import { applyPanelPositionFromSetting } from '@/features/core/layout/workbenchLayoutService';
import { resolveColorThemePreset } from './colorThemePresets';

/** Apply appearance settings that affect the workbench shell at runtime. */
export function useAppearanceSettings(): void {
  const colorTheme = useSettingsStore((s) => s.appearance.colorTheme);
  const smoothScroll = useSettingsStore((s) => s.appearance.smoothScroll);
  const panelPosition = useSettingsStore((s) => s.appearance.panelPosition);
  const updateTheme = useSettingsStore((s) => s.updateTheme);

  useEffect(() => {
    updateTheme(resolveColorThemePreset(colorTheme));
  }, [colorTheme, updateTheme]);

  useEffect(() => {
    document.documentElement.dataset.smoothScroll = smoothScroll ? 'true' : 'false';
  }, [smoothScroll]);

  useEffect(() => {
    applyPanelPositionFromSetting(panelPosition);
  }, [panelPosition]);
}
