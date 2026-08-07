import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { WorksheetService } from './worksheetService';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const projectInstanceId = '00000000-0000-0000-0000-000000000601';

beforeEach(() => {
  vi.clearAllMocks();
});

describe('WorksheetService database read lifecycle contract', () => {
  it('passes exact project identity to plot column reads', async () => {
    vi.mocked(invoke).mockResolvedValue({
      data: [{ x: 1, y: 2 }],
      xLabel: 'amount',
      yLabel: 'cost',
      xFormat: 'number',
      yFormat: 'number',
    });

    await WorksheetService.getPlotColumnPair(
      projectInstanceId,
      'sales',
      'amount',
      'cost',
      500,
    );

    expect(invoke).toHaveBeenCalledWith('get_plot_column_pair', {
      projectInstanceId,
      databaseId: 'sales',
      xCol: 'amount',
      yCol: 'cost',
      maxPoints: 500,
    });
  });
});
