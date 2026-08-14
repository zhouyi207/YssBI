import { beforeEach, describe, expect, it, vi } from 'vitest';
import { uiStore } from '@/features/core/ui/UIStore';
import { deleteResource } from '@/features/application/resource/resourceActions';
import { deleteGraphWithConfirm } from './deleteGraphWithConfirm';

vi.mock('@/features/application/resource/resourceActions', () => ({
  deleteResource: vi.fn(async () => undefined),
}));

describe('deleteGraphWithConfirm backend cascade', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('uses one confirmation and delegates function removal to the canonical backend cascade', async () => {
    const confirm = vi.spyOn(uiStore, 'confirm').mockResolvedValue(true);

    await expect(deleteGraphWithConfirm('functions/Target.yssbi-function', 'function'))
      .resolves.toBe(true);

    expect(confirm).toHaveBeenCalledOnce();
    expect(deleteResource).toHaveBeenCalledWith({
      id: 'functions/Target.yssbi-function',
      kind: 'function',
    });
  });

});
