import { describe, expect, it, vi, beforeEach } from 'vitest';
import { resolveInspectableSource, runtimePinRef, windowSourceRef } from './inspectableSource';

vi.mock('@/services/resultSource/resultSourceService', () => ({
  SourceService: {
    getPinDescriptor: vi.fn(),
    getDescriptor: vi.fn(),
  },
}));

import { SourceService } from '@/services/resultSource/resultSourceService';

const descriptor = {
  sourceId: 'runtime_live_events/Main_pin-a',
  kind: 'json' as const,
  presentation: { kind: 'inspector' as const },
  title: 'Live Table',
};

describe('resolveInspectableSource', () => {
  beforeEach(() => {
    vi.mocked(SourceService.getPinDescriptor).mockReset();
    vi.mocked(SourceService.getDescriptor).mockReset();
  });

  it('resolves runtime pins via pin index', async () => {
    vi.mocked(SourceService.getPinDescriptor).mockResolvedValue(descriptor);

    const result = await resolveInspectableSource(
      runtimePinRef('events/Main.yssbi-event', 'pin-a'),
    );

    expect(result).toEqual(descriptor);
    expect(SourceService.getPinDescriptor).toHaveBeenCalledWith(
      'events/Main.yssbi-event',
      'pin-a',
    );
    expect(SourceService.getDescriptor).not.toHaveBeenCalled();
  });

  it('resolves window sources via sourceId', async () => {
    vi.mocked(SourceService.getDescriptor).mockResolvedValue({
      ...descriptor,
      sourceId: 'window_abc',
    });

    const result = await resolveInspectableSource(windowSourceRef('window_abc'));

    expect(result?.sourceId).toBe('window_abc');
    expect(SourceService.getPinDescriptor).not.toHaveBeenCalled();
  });
});
