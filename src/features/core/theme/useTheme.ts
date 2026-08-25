// src/hooks/useTheme.ts
import { useSettingsStore } from "@/features/core/settings/settingsStore";
import { resolveThemeTokens } from '@/shared/theme/themeTokens';
import { useMemo } from 'react';

export const useTheme = () => {
    const theme = useSettingsStore((s) => s.theme);
    const updateTheme = useSettingsStore((s) => s.updateTheme);
    const tokens = useMemo(() => resolveThemeTokens(theme), [theme]);

    return { theme, tokens, updateTheme };
};
