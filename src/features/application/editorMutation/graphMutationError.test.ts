import { describe, expect, it } from 'vitest';
import { enUS } from '@/app/i18n/locales/en-US';
import { zhCN } from '@/app/i18n/locales/zh-CN';
import {
  GRAPH_MUTATION_ERROR_CODES,
  graphMutationErrorMessageKey,
  type GraphMutationErrorCode,
} from './graphMutationError';

const backendMessage = 'raw backend message for 00000000-0000-0000-0000-000000000123';

function localizedMessage(
  locale: typeof enUS | typeof zhCN,
  code: GraphMutationErrorCode,
): string {
  return locale.canvas.connection.errors[code];
}

describe('graphMutationErrorMessageKey', () => {
  it.each(GRAPH_MUTATION_ERROR_CODES)('maps %s to safe non-empty copy in both locales', (code) => {
    const key = graphMutationErrorMessageKey({ code, message: backendMessage });

    expect(key).toBe(`canvas.connection.errors.${code}`);
    for (const locale of [enUS, zhCN]) {
      const copy = localizedMessage(locale, code);
      expect(copy.trim()).not.toBe('');
      expect(copy).not.toContain(backendMessage);
      expect(copy).not.toContain('00000000-0000-0000-0000-000000000123');
    }
  });

  it.each([
    null,
    new Error(backendMessage),
    { code: 'internal_error', message: backendMessage },
    { code: 42, message: backendMessage },
  ])('returns null for an unrecognized rejection %#', (error) => {
    expect(graphMutationErrorMessageKey(error)).toBeNull();
  });
});
