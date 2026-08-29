import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ResultDescriptor } from '@/shared/types/domain/result';
import { loadPresentationWindow } from './loadPresentationWindow';

vi.mock('@/services/result/resultService', () => ({
  ResultService: {
    getDescriptor: vi.fn(),
    getValue: vi.fn(),
    getPage: vi.fn(),
  },
}));

vi.mock('@/features/application/observability/appLogger', () => ({
  logger: { app: { error: vi.fn() } },
}));

import { ResultService } from '@/services/result/resultService';

const provenance = {
  runId: '1',
  activationId: '2',
  graphPath: 'events/Main.yssbi-event',
  graphRevision: '7',
  nodeId: '00000000-0000-0000-0000-000000000002',
  output: null,
  createdAtMs: '1755072000000',
};

function descriptor(
  resultId: string,
  state: ResultDescriptor['state'],
  partial: Partial<ResultDescriptor> = {},
): ResultDescriptor {
  return {
    resultId,
    state,
    provenance,
    presentation: { kind: 'inspector' },
    valueKind: 'scalar',
    metadata: null,
    totalCount: state.kind === 'ready' ? 1 : null,
    title: 'Result',
    ...partial,
  };
}

describe('loadPresentationWindow', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it.each([
    ['17', descriptor('17', { kind: 'pending', progress: { completed: '2', total: '10' } }), 'pending'],
    ['18', descriptor('18', {
      kind: 'failed',
      failure: {
        code: 'execution_failed',
        cause: { kind: 'execution' },
        upstreamResultIds: [],
      },
    }), 'failed'],
    ['19', descriptor('19', { kind: 'cancelled' }), 'cancelled'],
  ] as const)('returns explicit %s descriptor state without fetching data', async (resultId, value, status) => {
    vi.mocked(ResultService.getDescriptor).mockResolvedValue(value);

    await expect(loadPresentationWindow(resultId)).resolves.toMatchObject({ status });
    expect(ResultService.getValue).not.toHaveBeenCalled();
    expect(ResultService.getPage).not.toHaveBeenCalled();
  });

  it('loads a ready scalar report as the canonical object only', async () => {
    const report = { title: 'OLS Summary', model_basic_info: {} };
    vi.mocked(ResultService.getDescriptor).mockResolvedValue(descriptor('20', { kind: 'ready' }, {
      presentation: { kind: 'report', report: 'olsSummary' },
    }));
    vi.mocked(ResultService.getValue).mockResolvedValue({ kind: 'value', value: report });

    await expect(loadPresentationWindow('20')).resolves.toMatchObject({
      status: 'ready',
      payload: { mode: 'report', report: 'olsSummary', data: report },
    });
    expect(ResultService.getValue).toHaveBeenCalledWith('20');
    expect(ResultService.getPage).not.toHaveBeenCalled();
  });

  it('requires report descriptors to use the scalar value kind', async () => {
    vi.mocked(ResultService.getDescriptor).mockResolvedValue(descriptor('21', { kind: 'ready' }, {
      presentation: { kind: 'report', report: 'olsSummary' },
      valueKind: 'sequence',
    }));
    vi.mocked(ResultService.getPage).mockResolvedValue({
      resultId: '21',
      offset: 0,
      requestedLimit: 200,
      actualCount: 1,
      totalCount: 1,
      hasMore: false,
      nextOffset: null,
      valueKind: 'sequence',
      metadata: null,
      values: [{ title: 'OLS Summary' }],
    });

    await expect(loadPresentationWindow('21')).resolves.toEqual({
      status: 'load_failed',
    });
  });

  it('loads ready paged inspector data through getPage only', async () => {
    vi.mocked(ResultService.getDescriptor).mockResolvedValue(descriptor('22', { kind: 'ready' }, {
      valueKind: 'dataSeries',
      totalCount: 2,
    }));
    vi.mocked(ResultService.getPage).mockResolvedValue({
      resultId: '22',
      offset: 0,
      requestedLimit: 200,
      actualCount: 2,
      totalCount: 2,
      hasMore: false,
      nextOffset: null,
      valueKind: 'dataSeries',
      metadata: null,
      values: [1, 2],
    });

    await expect(loadPresentationWindow('22')).resolves.toMatchObject({ status: 'ready' });
    expect(ResultService.getValue).not.toHaveBeenCalled();
    expect(ResultService.getPage).toHaveBeenCalledWith('22', 0, 200);
  });

  it('returns explicit missing states', async () => {
    await expect(loadPresentationWindow('')).resolves.toEqual({ status: 'missing_result_id' });
    vi.mocked(ResultService.getDescriptor).mockResolvedValue(null);
    await expect(loadPresentationWindow('999')).resolves.toEqual({ status: 'not_found' });
  });
});
