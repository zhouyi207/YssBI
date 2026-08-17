import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import {
  parsePinResultHistory,
  parseResultDescriptor,
  parseResultPage,
  parseResultValue,
} from '@/shared/types/dto/resultParser';
import { ResultService } from './resultService';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const output: PortAddressDto = {
  kind: 'declared',
  nodeId: '00000000-0000-0000-0000-000000000002',
  portKey: 'result',
};

const provenance = {
  runId: '9007199254740993',
  activationId: '9007199254740994',
  graphPath: 'events/contract.yssbi-event',
  graphRevision: '7',
  nodeId: '00000000-0000-0000-0000-000000000002',
  output: { graphPath: 'events/contract.yssbi-event', port: output },
  createdAtMs: '1755072000000',
};

const readyDescriptor = {
  resultId: '17',
  state: { kind: 'ready' as const },
  provenance,
  presentation: { kind: 'report' as const, report: 'olsSummary' as const },
  valueKind: 'scalar' as const,
  metadata: null,
  totalCount: 1,
  title: 'OLS Summary',
};

describe('result DTO parsers', () => {
  it('parses every descriptor state', () => {
    expect(parseResultDescriptor(readyDescriptor)).toEqual(readyDescriptor);
    expect(parseResultDescriptor({
      ...readyDescriptor,
      state: { kind: 'pending', progress: { completed: '2', total: '10' } },
      totalCount: null,
    }).state.kind).toBe('pending');
    expect(parseResultDescriptor({
      ...readyDescriptor,
      state: {
        kind: 'failed',
        failure: {
          code: 'upstream_failed',
          cause: { kind: 'upstream', upstreamResultId: '9' },
          upstreamResultIds: ['9'],
        },
      },
      totalCount: null,
    }).state.kind).toBe('failed');
    expect(parseResultDescriptor({
      ...readyDescriptor,
      state: { kind: 'cancelled' },
      totalCount: null,
    }).state.kind).toBe('cancelled');

  });

  it('strictly parses value, page, metadata, usage, and history variants', () => {
    expect(parseResultValue({ kind: 'value', value: { report: true } })).toEqual({
      kind: 'value', value: { report: true },
    });
    expect(parseResultValue({ kind: 'sequence', value: [1, 2] })).toEqual({
      kind: 'sequence', value: [1, 2],
    });
    expect(parseResultValue({ kind: 'dataSeries', value: [1, null] })).toEqual({
      kind: 'dataSeries', value: [1, null],
    });

    const page = {
      resultId: '17',
      offset: 0,
      requestedLimit: 2,
      actualCount: 2,
      totalCount: 3,
      hasMore: true,
      nextOffset: 2,
      valueKind: 'dataSeries' as const,
      metadata: {
        elementType: 'float64' as const,
        length: 3,
        nullCount: 1,
        name: 'x',
        format: null,
      },
      values: [1, null],
    };
    expect(parseResultPage(page)).toEqual(page);

    const history = [
      {
        resultId: '17',
        runId: '20',
        activationId: '21',
        graphRevision: '7',
        createdAtMs: '1755072000000',
        usage: { kind: 'produced' as const },
        state: { kind: 'ready' as const },
      },
      {
        resultId: '18',
        runId: '22',
        activationId: '23',
        graphRevision: '7',
        createdAtMs: '1755072000001',
        usage: { kind: 'reused' as const, originalActivationId: '21' },
        state: { kind: 'cancelled' as const },
      },
    ];
    expect(parsePinResultHistory(history)).toEqual(history);
    expect(() => parseResultPage({ ...page, limit: 2 })).toThrow();
    expect(() => parsePinResultHistory([{ ...history[0], unexpectedKey: true }])).toThrow();
  });
});

describe('ResultService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockResolvedValue(null);
  });

  it('invokes the exact result commands with decimal IDs and PortAddressDto output', async () => {
    vi.mocked(invoke).mockResolvedValueOnce(null)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce([]);
    await ResultService.getDescriptor('17');
    await ResultService.getValue('18');
    await ResultService.getPage('19', 10, 20);
    await ResultService.getPinHistory('events/contract.yssbi-event', output);

    expect(vi.mocked(invoke).mock.calls).toEqual([
      ['get_result_descriptor', { resultId: '17' }],
      ['get_result_value', { resultId: '18' }],
      ['get_result_page', { resultId: '19', offset: 10, limit: 20 }],
      ['get_pin_result_history', { graphPath: 'events/contract.yssbi-event', output }],
    ]);
    expect(invoke).not.toHaveBeenCalledWith(expect.stringContaining('release'), expect.anything());
  });

  it('parses command responses instead of trusting unknown IPC values', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce(readyDescriptor)
      .mockResolvedValueOnce({ kind: 'value', value: 4 })
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce([]);

    await expect(ResultService.getDescriptor('17')).resolves.toEqual(readyDescriptor);
    await expect(ResultService.getValue('17')).resolves.toEqual({ kind: 'value', value: 4 });
    await expect(ResultService.getPage('17', 0, 10)).resolves.toBeNull();
    await expect(ResultService.getPinHistory('events/contract.yssbi-event', output)).resolves.toEqual([]);
  });
});
