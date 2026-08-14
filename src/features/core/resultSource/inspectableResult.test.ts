import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ResultService } from '@/services/result/resultService';
import type { PinResultEntry } from '@/shared/types/dto/result';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import {
  outputPinRef,
  resolveInspectableResult,
  resolveInspectableResultRef,
  resultRef,
} from './inspectableResult';

vi.mock('@/services/result/resultService', () => ({
  ResultService: {
    getDescriptor: vi.fn(),
    getPinHistory: vi.fn(),
  },
}));

const graphPath = 'events/Main.yssbi-event';
const output: PortAddressDto = {
  kind: 'instance',
  nodeId: 'node-1',
  templateKey: 'values',
  instanceId: 'instance-2',
};

function entry(resultId: string, state: PinResultEntry['state']): PinResultEntry {
  return {
    resultId,
    runId: `run-${resultId}`,
    activationId: `activation-${resultId}`,
    graphRevision: '7',
    createdAtMs: resultId,
    usage: { kind: 'produced' },
    state,
  };
}

describe('resolveInspectableResult', () => {
  beforeEach(() => vi.clearAllMocks());

  it('resolves an exact result ID', async () => {
    vi.mocked(ResultService.getDescriptor).mockResolvedValue(null);
    await expect(resolveInspectableResult(resultRef('17'))).resolves.toBeNull();
    expect(ResultService.getDescriptor).toHaveBeenCalledWith('17');
  });

  it.each([
    ['pending', { kind: 'pending', progress: { completed: '0', total: null } }],
    ['failed', {
      kind: 'failed',
      failure: {
        code: 'executionFailed',
        message: 'failed',
        cause: { kind: 'execution' },
        upstreamResultIds: [],
      },
    }],
    ['cancelled', { kind: 'cancelled' }],
  ] satisfies ReadonlyArray<readonly [string, PinResultEntry['state']]>)('selects the latest occurrence even when it is %s', async (_label, state) => {
    vi.mocked(ResultService.getPinHistory).mockResolvedValue([
      entry('17', { kind: 'ready' }),
      entry('18', state),
    ]);

    await expect(resolveInspectableResultRef(outputPinRef(graphPath, output))).resolves.toEqual({
      ref: resultRef('18'),
      history: expect.objectContaining({
        graphPath,
        output,
        selectedResultId: '18',
      }),
    });
    expect(ResultService.getPinHistory).toHaveBeenCalledWith(graphPath, output);
  });

  it('selects an exact historical result ID', async () => {
    vi.mocked(ResultService.getPinHistory).mockResolvedValue([
      entry('17', { kind: 'ready' }),
      entry('18', { kind: 'cancelled' }),
    ]);

    await expect(resolveInspectableResultRef(
      outputPinRef(graphPath, output),
      '17',
    )).resolves.toEqual({
      ref: resultRef('17'),
      history: expect.objectContaining({ selectedResultId: '17' }),
    });
  });

  it('rejects a historical result ID that is not in that output history', async () => {
    vi.mocked(ResultService.getPinHistory).mockResolvedValue([entry('17', { kind: 'ready' })]);
    await expect(resolveInspectableResultRef(
      outputPinRef(graphPath, output),
      '99',
    )).resolves.toEqual({
      ref: null,
      history: expect.objectContaining({ selectedResultId: null }),
    });
  });
});
