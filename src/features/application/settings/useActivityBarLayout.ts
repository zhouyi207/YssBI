import { useMemo } from 'react';
import { useSettingsStore } from '@/features/core/settings/settingsStore';
import { resolveActivityBarLayout } from './appearanceRuntime';

export function useActivityBarLayout(zenMode: boolean) {
  const activityBarPosition = useSettingsStore((s) => s.appearance.activityBarPosition);
  return useMemo(
    () => resolveActivityBarLayout(activityBarPosition, zenMode),
    [activityBarPosition, zenMode],
  );
}
