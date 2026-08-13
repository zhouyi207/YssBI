import { beforeEach, describe, expect, it, vi } from 'vitest';
import { LogService } from '@/services/log';
import { LogLevel, LogType } from '@/shared/types/ui';
import { uiStore } from './UIStore';

vi.mock('@/services/log', () => ({
  LogService: {
    frontendLog: vi.fn().mockResolvedValue(undefined),
  },
}));

describe('UIStore notifications', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it.each([
    ['success', LogLevel.Info],
    ['info', LogLevel.Info],
    ['warning', LogLevel.Warn],
    ['error', LogLevel.Error],
  ] as const)('routes %s feedback to the Notify log', (type, level) => {
    uiStore.showToast('Operation feedback', type);

    expect(LogService.frontendLog).toHaveBeenCalledWith(
      level,
      LogType.Notify,
      'Operation feedback',
      'UI',
    );
  });
});
