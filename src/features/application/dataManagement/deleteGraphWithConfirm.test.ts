import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invalidateGraphProjections } from '@/features/application/editorProjection/graphProjectionCoordinator';
import { uiStore } from '@/features/core/ui/UIStore';
import { GraphService } from '@/services/graph/graphService';
import { deleteResource } from '@/features/application/resource/resourceActions';
import { deleteGraphWithConfirm } from './deleteGraphWithConfirm';

vi.mock('@/features/application/editorProjection/graphProjectionCoordinator', () => ({
  invalidateGraphProjections: vi.fn(async () => undefined),
}));

vi.mock('@/features/application/resource/resourceActions', () => ({
  deleteResource: vi.fn(async () => undefined),
}));

describe('deleteGraphWithConfirm projection refresh', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('refreshes purged caller projections without consuming returned graph bodies', async () => {
    vi.spyOn(uiStore, 'confirm3').mockResolvedValue('discard');
    vi.spyOn(GraphService, 'getFunctionCallSites').mockResolvedValue([
      { callerGraphPath: 'events/Caller.yssbi-event', nodeIds: ['call-1'] },
    ]);
    vi.spyOn(GraphService, 'purgeFunctionCallSites').mockResolvedValue([
      {
        path: 'events/Caller.yssbi-event',
        name: 'Caller',
        type: 'event',
        nodes: [],
        pins: [],
        connections: { connections: [] },
      },
    ]);

    await expect(deleteGraphWithConfirm('functions/Target.yssbi-function', 'function'))
      .resolves.toBe(true);

    expect(invalidateGraphProjections).toHaveBeenCalledWith([
      'events/Caller.yssbi-event',
    ]);
    expect(deleteResource).toHaveBeenCalledWith({
      id: 'functions/Target.yssbi-function',
      kind: 'function',
    });
  });
});
