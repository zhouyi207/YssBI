import { describe, expect, it } from 'vitest';
import { enUS } from '@/app/i18n/locales/en-US';
import { zhCN } from '@/app/i18n/locales/zh-CN';

describe('GraphTraceDetails localization', () => {
  it('provides complete English and Chinese trace projection labels', () => {
    expect(Object.keys(zhCN.detail.trace)).toEqual(Object.keys(enUS.detail.trace));
  });
});
