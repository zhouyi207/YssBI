// src/hooks/useTheme.ts
import { useSettingsStore } from "@/features/core/settings/settingsStore";
import { useShallow } from 'zustand/react/shallow';

export const useTheme = () => {
    return useSettingsStore(useShallow((s) => ({
        theme: s.theme,
        updateTheme: s.updateTheme,
    })));
};
