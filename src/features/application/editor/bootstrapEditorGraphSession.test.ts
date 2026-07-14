import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@/features/core/ui/UIStore', () => ({
  uiStore: {
    showToast: vi.fn(),
  },
}));

vi.mock('./switchEditorTab', () => ({
  activateCurrentEditorTab: vi.fn(),
}));

import { uiStore } from '@/features/core/ui/UIStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { activateCurrentEditorTab } from './switchEditorTab';
import { bootstrapEditorGraphSession } from './bootstrapEditorGraphSession';

describe('bootstrapEditorGraphSession', () => {
  function seedActiveGraphTab(): void {
    useEditorTabStore.getState().initGroupPlacement('default_editor', [
      { id: 'events/test.yssbi-event', component: 'GraphEditor', type: 'event' },
    ]);
  }

  beforeEach(() => {
    vi.clearAllMocks();
    useEditorTabStore.setState({ registry: {}, placements: {} });
  });

  it('does not warn when the restored group has no active tab', async () => {
    const ok = await bootstrapEditorGraphSession('default_editor');

    expect(ok).toBe(true);
    expect(activateCurrentEditorTab).not.toHaveBeenCalled();
    expect(uiStore.showToast).not.toHaveBeenCalled();
  });

  it('returns true on first successful activation', async () => {
    seedActiveGraphTab();
    vi.mocked(activateCurrentEditorTab).mockResolvedValue(true);

    const ok = await bootstrapEditorGraphSession('default_editor', {
      maxAttempts: 3,
      retryDelayMs: 0,
    });

    expect(ok).toBe(true);
    expect(activateCurrentEditorTab).toHaveBeenCalledTimes(1);
    expect(uiStore.showToast).not.toHaveBeenCalled();
  });

  it('retries transient failures and surfaces a toast when all attempts fail', async () => {
    seedActiveGraphTab();
    vi.mocked(activateCurrentEditorTab).mockResolvedValue(false);

    const ok = await bootstrapEditorGraphSession('default_editor', {
      maxAttempts: 3,
      retryDelayMs: 0,
    });

    expect(ok).toBe(false);
    expect(activateCurrentEditorTab).toHaveBeenCalledTimes(3);
    expect(uiStore.showToast).toHaveBeenCalledWith(
      '当前编辑器图未能加载，请重新点击标签页或画布',
      'warning',
      4000,
    );
  });
});
