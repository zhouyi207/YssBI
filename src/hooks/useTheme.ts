// src/hooks/useTheme.ts
import { useSettingsStore } from "@/stores/settingsStore";

export const useTheme = () => {
    return useSettingsStore((s) => ({
        theme: s.theme,
        updateTheme: s.updateTheme,
    }));
};
