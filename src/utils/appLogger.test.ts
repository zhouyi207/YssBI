import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  submit: vi.fn(),
}));

vi.mock('@/services/log', () => ({
  LogService: { submitFrontendDiagnostics: mocks.submit },
}));

import { FRONTEND_DIAGNOSTIC_BATCH_MAX_DELAY_MS } from '@/app/appConfig/default';
import { logger } from './appLogger';

describe('appLogger', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    mocks.submit.mockReset().mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it('writes explicit logs once and never forwards raw console calls', async () => {
    const consoleLog = vi.spyOn(console, 'log').mockImplementation(() => {});

    logger.app.info('application ready', 'Bootstrap');
    expect(consoleLog).toHaveBeenCalledOnce();
    expect(consoleLog).toHaveBeenCalledWith('[APP][Bootstrap] application ready');

    await vi.advanceTimersByTimeAsync(FRONTEND_DIAGNOSTIC_BATCH_MAX_DELAY_MS);
    expect(mocks.submit).toHaveBeenCalledOnce();
    expect(mocks.submit.mock.calls[0]?.[0]).toMatchObject([{
      level: 'info',
      domain: 'application',
      target: 'Bootstrap',
      message: 'application ready',
      source: 'Bootstrap',
      fields: {},
    }]);

    console.log('raw console only');
    await vi.advanceTimersByTimeAsync(FRONTEND_DIAGNOSTIC_BATCH_MAX_DELAY_MS);
    expect(mocks.submit).toHaveBeenCalledOnce();
  });

  it('does not expose a user-notification logging channel', () => {
    expect(logger).not.toHaveProperty('notify');
  });
});
