import { describe, expect, it } from 'vitest';
import { DEFAULT_DARK_THEME } from './settings';

describe('default dark theme', () => {
  it('uses layered neutral surfaces with a Codex-style green accent', () => {
    expect(DEFAULT_DARK_THEME.workbenchBackground).toBe('#171717');
    expect(DEFAULT_DARK_THEME.sidebarBackground).toBe('#202020');
    expect(DEFAULT_DARK_THEME.nodeBase).toBe('#242424');
    expect(DEFAULT_DARK_THEME.accentColor).toBe('#10a37f');
    expect(DEFAULT_DARK_THEME.selectionRegion).toBe('#10a37f');
  });
});
