import { describe, expect, it } from 'vitest';
import { enUS } from '@/app/i18n/locales/en-US';
import { zhCN } from '@/app/i18n/locales/zh-CN';

describe('GraphTraceDetails localization', () => {
  it('provides complete English and Chinese trace projection labels', () => {
    expect(enUS.detail.trace).toMatchObject({
      title: 'Developer trace',
      refresh: 'Refresh',
      loading: 'Loading trace…',
      runNotFound: 'Run trace is no longer retained',
      redacted: '[redacted]',
    });
    expect(zhCN.detail.trace).toMatchObject({
      title: '开发者跟踪',
      refresh: '刷新',
      loading: '正在加载跟踪…',
      runNotFound: '运行跟踪已不在保留范围内',
      redacted: '[已脱敏]',
    });
    expect(Object.keys(zhCN.detail.trace)).toEqual(Object.keys(enUS.detail.trace));
  });
});
