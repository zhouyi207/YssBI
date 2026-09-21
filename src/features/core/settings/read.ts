import { createReadProjection, useReadProjection } from "@/features/core/state/readProjection";

import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { resolveColorThemePreset, type ThemePalette } from "@/shared/theme/colorThemePresets";
import { useSettingsStore } from "./settingsStore";
import type { AiSettings, AppSettings, AppearanceSettings } from "@/shared/types/settings";

export interface SettingsReadSnapshot {
  readonly ai: DeepReadonly<AiSettings>;
  readonly theme: ThemePalette;
  readonly appearance: DeepReadonly<AppearanceSettings>;
  readonly isLoading: boolean;
}

function buildSnapshot(): DeepReadonly<SettingsReadSnapshot> {
  const state = useSettingsStore.getState();
  return {
    ai: state.ai,
    appearance: state.appearance,
    isLoading: state.isLoading,
    // Reuse the immutable preset so unrelated preferences do not invalidate theme consumers.
    theme: resolveColorThemePreset(state.appearance.colorTheme),
  };
}

const projection = createReadProjection(buildSnapshot, [useSettingsStore]);
export const getSettingsSnapshot = projection.getSnapshot;
export const subscribeSettingsRead = projection.subscribe;
export function useSettingsRead<T>(
  selector: (snapshot: DeepReadonly<SettingsReadSnapshot>) => T,
): T {
  return useReadProjection(projection, selector);
}

export type { AppSettings };
