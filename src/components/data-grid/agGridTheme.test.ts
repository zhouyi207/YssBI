import { describe, expect, it } from 'vitest';
import { DEFAULT_LIGHT_THEME } from '@/app/appConfig/default';
import { resolveThemeTokens } from '@/shared/theme/themeTokens';
import { getAgGridThemeParams } from './agGridTheme';

describe('getAgGridThemeParams', () => {
  it('maps the semantic runtime tokens to all grid surfaces and text roles', () => {
    const settings = {
      ...DEFAULT_LIGHT_THEME,
      workbenchBackground: '#f0f4f8',
      sidebarBackground: '#e5ebf2',
      foreground: '#172033',
      mutedForeground: '#64748b',
      accentColor: '#2563eb',
      borderColor: '#cbd5e1',
    };
    const tokens = resolveThemeTokens(settings);
    const params = getAgGridThemeParams(settings);

    expect(params.backgroundColor).toBe(tokens.workbenchBg);
    expect(params.dataBackgroundColor).toBe(tokens.workbenchBg);
    expect(params.headerBackgroundColor).toBe(tokens.panelHeaderBg);
    expect(params.cellTextColor).toBe(tokens.foreground);
    expect(params.subtleTextColor).toBe(tokens.mutedForeground);
    expect(params.borderColor).toBe(tokens.border);
    expect(params.rowHoverColor).toBe(tokens.accentSoft);
    expect(params.accentColor).toBe(tokens.accent);
  });
});
