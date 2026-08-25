// @vitest-environment happy-dom

import { describe, expect, it } from 'vitest';
import { DEFAULT_DARK_THEME } from '@/app/appConfig/default';
import { resolveThemeTokens } from '@/shared/theme/themeTokens';
import { applyThemeTokens } from './SettingsEffectsProvider';

describe('applyThemeTokens', () => {
  it('writes the resolved semantic and compatibility variables as one set', () => {
    const root = document.createElement('html');
    const tokens = resolveThemeTokens(DEFAULT_DARK_THEME);

    applyThemeTokens(root, tokens);

    expect(root.style.getPropertyValue('--background')).toBe(tokens.workbenchBg);
    expect(root.style.getPropertyValue('--foreground')).toBe(tokens.foreground);
    expect(root.style.getPropertyValue('--muted')).toBe(tokens.surfaceSunken);
    expect(root.style.getPropertyValue('--secondary')).toBe(tokens.surfaceRaised);
    expect(root.style.getPropertyValue('--primary')).toBe(tokens.accent);
    expect(root.style.getPropertyValue('--primary-foreground')).toBe(tokens.primaryForeground);
    expect(root.style.getPropertyValue('--border')).toBe(tokens.border);
    expect(root.style.getPropertyValue('--grid-lines')).toBe(tokens.grid);
    expect(root.style.getPropertyValue('--selection-region')).toBe(tokens.selection);
    expect(root.style.getPropertyValue('--pin-exec')).toBe(tokens.pins.exec);
    expect(root.style.getPropertyValue('--pin-table')).toBe(tokens.pins.table);
  });
});
