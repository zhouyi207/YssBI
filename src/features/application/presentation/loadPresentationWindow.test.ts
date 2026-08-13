import { describe, expect, it, vi, beforeEach } from 'vitest';
import { loadPresentationWindow } from './loadPresentationWindow';

vi.mock('@/services/resultSource/resultSourceService', () => ({
  SourceService: {
    getDescriptor: vi.fn(),
    getValue: vi.fn(),
  },
}));

import { SourceService } from '@/services/resultSource/resultSourceService';

const inspectorDescriptor = {
  sourceId: 'runtime_live_events/Main_pin-a',
  kind: 'json' as const,
  presentation: { kind: 'inspector' as const },
  title: 'Result Table',
};

describe('loadPresentationWindow', () => {
  beforeEach(() => {
    vi.mocked(SourceService.getDescriptor).mockReset();
    vi.mocked(SourceService.getValue).mockReset();
  });

  it('loads inspector window by sourceId', async () => {
    vi.mocked(SourceService.getDescriptor).mockResolvedValue(inspectorDescriptor);

    const state = await loadPresentationWindow('runtime_live_events/Main_pin-a');

    expect(state.status).toBe('ready');
    if (state.status === 'ready') {
      expect(state.payload.mode).toBe('inspector');
    }
  });

  it('unwraps a single report captured by View Data from its sequence source', async () => {
    const report = { title: 'OLS Summary', model_basic_info: {} };
    vi.mocked(SourceService.getDescriptor).mockResolvedValue({
      sourceId: 'view-source',
      kind: 'json',
      presentation: { kind: 'report', report: 'olsSummary' },
      title: 'Results',
    });
    vi.mocked(SourceService.getValue).mockResolvedValue({
      kind: 'sequence',
      value: [report],
    });

    const state = await loadPresentationWindow('view-source');

    expect(state.status).toBe('ready');
    if (state.status === 'ready') {
      expect(state.payload).toEqual({
        mode: 'report',
        report: 'olsSummary',
        data: report,
      });
    }
  });

  it('returns not_found when sourceId is unknown', async () => {
    vi.mocked(SourceService.getDescriptor).mockResolvedValue(null);

    const state = await loadPresentationWindow('missing');

    expect(state.status).toBe('not_found');
  });
});
