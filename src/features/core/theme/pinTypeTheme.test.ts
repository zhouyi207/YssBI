import { describe, expect, it } from 'vitest';
import { DEFAULT_DARK_THEME } from '@/app/appConfig/default';
import { resolveThemeTokens } from '@/shared/theme/themeTokens';
import { getPinTypeCategory, getPinTypeColor } from './pinTypeTheme';

describe('pin type semantic palette', () => {
  const tokens = resolveThemeTokens(DEFAULT_DARK_THEME);

  it('maps numeric aliases to the numeric category', () => {
    expect(getPinTypeCategory('Int32')).toBe('numeric');
    expect(getPinTypeCategory('Float64')).toBe('numeric');
    expect(getPinTypeColor('float32', tokens)).toBe(tokens.pins.numeric);
  });

  it('maps table, temporal, text, boolean, and unknown aliases', () => {
    expect(getPinTypeCategory('DataFrame')).toBe('table');
    expect(getPinTypeCategory('DateTime')).toBe('temporal');
    expect(getPinTypeCategory('String')).toBe('text');
    expect(getPinTypeCategory('Boolean')).toBe('boolean');
    expect(getPinTypeCategory('unknown')).toBe('object');
    expect(getPinTypeColor('DataSeries', tokens)).toBe(tokens.pins.table);
  });
});
