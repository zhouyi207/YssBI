import { beforeEach, describe, expect, it } from 'vitest';
import type { GraphOutputRefDto, ResultDescriptor } from '@/shared/types/dto/result';
import {
  resultWorkspaceTabKey,
  useResultWorkspaceStore,
} from './resultWorkspaceStore';

const outputA: GraphOutputRefDto = {
  graphPath: 'events/Main.yssbi-event',
  port: { kind: 'declared', nodeId: 'node-a', portKey: 'result' },
};

const outputB: GraphOutputRefDto = {
  graphPath: 'events/Main.yssbi-event',
  port: { kind: 'instance', nodeId: 'node-b', templateKey: 'value', instanceId: '2' },
};

function descriptor(
  resultId: string,
  output: GraphOutputRefDto | null,
  title = `Result ${resultId}`,
): ResultDescriptor {
  return {
    resultId,
    state: { kind: 'ready' },
    provenance: {
      runId: `run-${resultId}`,
      activationId: `activation-${resultId}`,
      graphPath: output?.graphPath ?? 'events/Main.yssbi-event',
      graphRevision: '7',
      nodeId: output?.port.nodeId ?? 'node-without-output',
      output,
      createdAtMs: '1787270400000',
    },
    presentation: { kind: 'inspector' },
    valueKind: 'scalar',
    metadata: null,
    totalCount: null,
    title,
  };
}

describe('resultWorkspaceStore', () => {
  beforeEach(() => useResultWorkspaceStore.getState().reset());

  it('updates one tab when the same output produces a newer result', () => {
    const firstKey = useResultWorkspaceStore.getState().openResult(descriptor('1', outputA));
    const secondKey = useResultWorkspaceStore.getState().openResult(descriptor('2', outputA));
    const state = useResultWorkspaceStore.getState();

    expect(secondKey).toBe(firstKey);
    expect(state.order).toEqual([firstKey]);
    expect(state.tabs[firstKey]).toMatchObject({ resultId: '2', title: 'Result 2' });
    expect(state.activeTabKey).toBe(firstKey);
  });

  it('keeps different outputs and output-less results isolated', () => {
    const outputAKey = useResultWorkspaceStore.getState().openResult(descriptor('1', outputA));
    const outputBKey = useResultWorkspaceStore.getState().openResult(descriptor('2', outputB));
    const noOutput1 = useResultWorkspaceStore.getState().openResult(descriptor('3', null));
    const noOutput2 = useResultWorkspaceStore.getState().openResult(descriptor('4', null));

    expect(new Set([outputAKey, outputBKey, noOutput1, noOutput2]).size).toBe(4);
    expect(useResultWorkspaceStore.getState().order).toHaveLength(4);
  });

  it('closes the active tab into its adjacent tab and supports reordering', () => {
    const a = useResultWorkspaceStore.getState().openResult(descriptor('1', outputA));
    const b = useResultWorkspaceStore.getState().openResult(descriptor('2', outputB));
    const c = useResultWorkspaceStore.getState().openResult(descriptor('3', null));

    useResultWorkspaceStore.getState().moveTab(c, a);
    expect(useResultWorkspaceStore.getState().order).toEqual([c, a, b]);

    useResultWorkspaceStore.getState().closeTab(c);
    expect(useResultWorkspaceStore.getState().order).toEqual([a, b]);
    expect(useResultWorkspaceStore.getState().activeTabKey).toBe(a);
  });

  it('resets all runtime-only tab metadata', () => {
    useResultWorkspaceStore.getState().openResult(descriptor('1', outputA));
    useResultWorkspaceStore.getState().reset();
    expect(useResultWorkspaceStore.getState()).toMatchObject({
      order: [],
      activeTabKey: null,
      tabs: {},
    });
  });

  it('uses result identity when provenance has no output', () => {
    expect(resultWorkspaceTabKey(descriptor('17', null))).toBe('result:2:17');
  });
});
