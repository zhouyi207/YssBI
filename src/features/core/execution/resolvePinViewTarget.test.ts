import { describe, expect, it } from 'vitest';
import type { PinResultState } from '@/shared/types/ui';
import { pinResultCacheKey } from './pinResultIndex';
import {
  evaluatePinViewState,
  inspectableRefsFromPinView,
  resolveUpstreamPinIds,
} from './pinViewTarget';

function result(pinId: string, graphPath = 'g1'): PinResultState {
  return {
    graphPath,
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

function pinResultsMap(
  entries: Array<{ graphPath: string; pinId: string; state?: PinResultState }>,
): Map<string, PinResultState> {
  return new Map(
    entries.map(({ graphPath, pinId, state }) => [
      pinResultCacheKey(graphPath, pinId),
      state ?? result(pinId, graphPath),
    ]),
  );
}

describe('pinViewTarget', () => {
  it('evaluatePinViewState resolves output pin from cache', () => {
    const state = evaluatePinViewState({
      graphPath: 'g1',
      pinId: 'out-1',
      direction: 'output',
      isExec: false,
      pinResults: pinResultsMap([{ graphPath: 'g1', pinId: 'out-1' }]),
    });
    expect(state.enabled).toBe(true);
    expect(state.refs[0]).toEqual({ kind: 'runtimePin', graphPath: 'g1', pinId: 'out-1' });
  });

  it('resolves connected input via upstream cache', () => {
    const state = evaluatePinViewState({
      graphPath: 'g1',
      pinId: 'in-1',
      direction: 'input',
      isExec: false,
      connectionIds: ['out-1->in-1'],
      pinResults: pinResultsMap([{ graphPath: 'g1', pinId: 'out-1' }]),
    });
    expect(state.enabled).toBe(true);
    expect(state.refs[0]?.kind).toBe('runtimePin');
    if (state.refs[0]?.kind === 'runtimePin') {
      expect(state.refs[0].pinId).toBe('out-1');
    }
  });

  it('hides view for exec and unconnected input', () => {
    expect(
      evaluatePinViewState({
        graphPath: 'g1',
        pinId: 'exec',
        direction: 'input',
        isExec: true,
      }).showMenu,
    ).toBe(false);
    expect(
      evaluatePinViewState({
        graphPath: 'g1',
        pinId: 'in-1',
        direction: 'input',
        isExec: false,
      }).showMenu,
    ).toBe(false);
  });

  it('shows disabled view for output without result', () => {
    expect(
      evaluatePinViewState({
        graphPath: 'g1',
        pinId: 'out-1',
        direction: 'output',
        isExec: false,
        pinResults: new Map(),
      }).disabledReason,
    ).toBe('no_run');
  });

  it('resolveUpstreamPinIds follows connection id', () => {
    expect(resolveUpstreamPinIds('in-1', ['out-1->in-1'])).toEqual(['out-1']);
  });

  it('inspectableRefsFromPinView prefers cached graphPath for nested calls', () => {
    const pinResults = pinResultsMap([
      {
        graphPath: 'functions/Helper.yssbi-function',
        pinId: 'out-1',
        state: result('out-1', 'functions/Helper.yssbi-function'),
      },
    ]);

    const refs = inspectableRefsFromPinView({
      graphPath: 'events/Main.yssbi-event',
      pinId: 'out-1',
      direction: 'output',
      isExec: false,
      pinResults,
    });

    expect(refs).toEqual([
      {
        kind: 'runtimePin',
        graphPath: 'functions/Helper.yssbi-function',
        pinId: 'out-1',
      },
    ]);
  });

  it('evaluatePinViewState enables completed run without cache', () => {
    const idle = evaluatePinViewState({
      graphPath: 'g1',
      pinId: 'out-1',
      direction: 'output',
      isExec: false,
      pinResults: new Map(),
      executionStatus: 'idle',
    });
    expect(idle.enabled).toBe(false);

    const completed = evaluatePinViewState({
      graphPath: 'g1',
      pinId: 'out-1',
      direction: 'output',
      isExec: false,
      pinResults: new Map(),
      executionStatus: 'completed',
    });
    expect(completed.enabled).toBe(true);
  });
});
