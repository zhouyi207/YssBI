import { beforeEach, describe, expect, it, vi } from 'vitest';


vi.mock('./switchEditorTab', () => ({
  activateCurrentEditorTab: vi.fn(),
}));

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

  it('returns true when the restored group has no active tab', async () => {
    const ok = await bootstrapEditorGraphSession('default_editor');

    expect(ok).toBe(true);
    expect(activateCurrentEditorTab).not.toHaveBeenCalled();
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
  });

  it('retries transient failures and returns false when all attempts fail', async () => {
    seedActiveGraphTab();
    vi.mocked(activateCurrentEditorTab).mockResolvedValue(false);

    const ok = await bootstrapEditorGraphSession('default_editor', {
      maxAttempts: 3,
      retryDelayMs: 0,
    });

    expect(ok).toBe(false);
    expect(activateCurrentEditorTab).toHaveBeenCalledTimes(3);
  });
});
