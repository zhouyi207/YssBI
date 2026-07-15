import { describe, expect, it } from 'vitest';
import { getNextColorThemePreset } from './colorThemePresets';

describe('color theme presets', () => {
  it('cycles unknown and known presets through the canonical order', () => {
    expect(getNextColorThemePreset('Dark Modern (Default)')).toBe('OLED Black');
    expect(getNextColorThemePreset('OLED Black')).toBe('Light Modern');
    expect(getNextColorThemePreset('Light Modern')).toBe('Dark Modern (Default)');
    expect(getNextColorThemePreset('custom')).toBe('Dark Modern (Default)');
  });
});
