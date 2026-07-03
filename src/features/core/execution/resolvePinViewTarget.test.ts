import { describe, expect, it } from 'vitest';
import type { PinResultState } from '@/shared/types/ui';
import {
  resolvePinViewDisabledReason,
  resolvePinViewTargetFromCache,
  resolveUpstreamPinIds,
  shouldShowPinViewMenuItem,
} from './resolvePinViewTarget';

function result(pinId: string): PinResultState {
  return {
    graphId: 'g1',
    nodeId: 'n1',
    pinId,
    sourceId: `src-${pinId}`,
    descriptor: {
      sourceId: `src-${pinId}`,
      kind: 'scalar',
      presentation: { kind: 'inspector' },
      title: 'Result',
    },
  };
}

describe('resolvePinViewTarget', () => {
  it('resolves output pin from cache', () => {
    const pinResults = new Map([['out-1', result('out-1')]]);
    const target = resolvePinViewTargetFromCache({
      graphId: 'g1',
      pinId: 'out-1',
      direction: 'output',
      pinType: 'float',
      pinResults,
    });
    expect(target?.sourcePinId).toBe('out-1');
  });

  it('resolves connected input via upstream cache', () => {
    const pinResults = new Map([['out-1', result('out-1')]]);
    const target = resolvePinViewTargetFromCache({
      graphId: 'g1',
      pinId: 'in-1',
      direction: 'input',
      pinType: 'float',
      connectionIds: ['out-1->in-1'],
      pinResults,
    });
    expect(target?.sourcePinId).toBe('out-1');
  });

  it('hides view for exec and unconnected input', () => {
    expect(
      shouldShowPinViewMenuItem({
        graphId: 'g1',
        pinId: 'exec',
        direction: 'input',
        pinType: 'exec',
      }),
    ).toBe(false);
    expect(
      shouldShowPinViewMenuItem({
        graphId: 'g1',
        pinId: 'in-1',
        direction: 'input',
        pinType: 'float',
      }),
    ).toBe(false);
  });

  it('shows disabled view for output without result', () => {
    expect(
      resolvePinViewDisabledReason({
        graphId: 'g1',
        pinId: 'out-1',
        direction: 'output',
        pinType: 'float',
        pinResults: new Map(),
      }),
    ).toBe('no_run');
  });

  it('resolveUpstreamPinIds follows connection id', () => {
    expect(resolveUpstreamPinIds('in-1', ['out-1->in-1'])).toEqual(['out-1']);
  });
});
