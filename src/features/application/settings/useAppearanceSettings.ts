import { useEffect } from 'react';
import { useSettingsStore } from '@/features/core/settings/settingsStore';
import { applyPanelPositionFromSetting } from '@/features/core/layout/workbenchLayoutService';

/** Editor workbench shell: panel dock position (requires layout store). */
export function useEditorWorkbenchAppearance(): void {
  const panelPosition = useSettingsStore((s) => s.appearance.panelPosition);

  useEffect(() => {
    applyPanelPositionFromSetting(panelPosition);
  }, [panelPosition]);
}

/** @deprecated Use `useEditorWorkbenchAppearance` — global appearance runs in SettingsEffectsProvider. */
export const useAppearanceSettings = useEditorWorkbenchAppearance;
