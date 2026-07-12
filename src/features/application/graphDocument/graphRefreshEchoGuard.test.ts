import { describe, expect, it } from 'vitest';
import {
  markGraphRefreshEcho,
  resolveGraphRefreshEcho,
  shouldSuppressGraphRefreshEcho,
} from './graphRefreshEchoGuard';

describe('graphRefreshEchoGuard', () => {
  it('suppresses graph refresh echo while marked pending', () => {
    expect(shouldSuppressGraphRefreshEcho('functions/A.yssbi-function')).toBe(false);

    markGraphRefreshEcho(['functions/A.yssbi-function']);
    expect(shouldSuppressGraphRefreshEcho('functions/A.yssbi-function')).toBe(true);

    resolveGraphRefreshEcho(['functions/A.yssbi-function']);
    expect(shouldSuppressGraphRefreshEcho('functions/A.yssbi-function')).toBe(false);
  });
});
