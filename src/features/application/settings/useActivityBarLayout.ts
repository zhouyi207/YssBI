import { useMemo } from 'react';
import { useSettingsStore } from '@/features/core/settings/settingsStore';
import { resolveActivityBarLayout } from './appearanceRuntime';

export function useActivityBarLayout() {
  const activityBarPosition = useSettingsStore((state) => state.appearance.activityBarPosition);
  return useMemo(
    () => resolveActivityBarLayout(activityBarPosition),
    [activityBarPosition],
  );
}
