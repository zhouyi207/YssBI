// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useResultWorkspaceStore } from '@/features/core/resultWorkspace';
import type { GraphOutputRefDto, ResultDescriptor } from '@/shared/types/dto/result';
import { ResultPane } from './ResultPane';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('./ResultTabStrip', () => ({
  ResultTabStrip: () => null,
}));

vi.mock('./ResultContent', async () => {
  const { useState } = await import('react');
  return {
    ResultContent: ({ resultId }: { resultId: string }) => {
      const [mountedResultId] = useState(resultId);
      return (
        <div
          data-testid="result-content"
          data-result-id={resultId}
          data-mounted-result-id={mountedResultId}
        />
      );
    },
  };
});

const output: GraphOutputRefDto = {
  graphPath: 'events/Main.yssbi-event',
  port: { kind: 'declared', nodeId: 'node-a', portKey: 'result' },
};

function descriptor(resultId: string): ResultDescriptor {
  return {
    resultId,
    state: { kind: 'ready' },
    provenance: {
      runId: `run-${resultId}`,
      activationId: `activation-${resultId}`,
      graphPath: output.graphPath,
      graphRevision: '1',
      nodeId: output.port.nodeId,
      output,
      createdAtMs: '1787270400000',
    },
    presentation: { kind: 'inspector' },
    valueKind: 'scalar',
    metadata: null,
    totalCount: null,
    title: `Result ${resultId}`,
  };
}

describe('ResultPane', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
      .IS_REACT_ACT_ENVIRONMENT = true;
    useResultWorkspaceStore.getState().reset();
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    useResultWorkspaceStore.getState().reset();
  });

  it('remounts content when the same output is replaced by a new result identity', () => {
    useResultWorkspaceStore.getState().openResult(descriptor('result-a'));
    act(() => root.render(<ResultPane />));

    expect(container.querySelector('[data-testid="result-content"]')).toMatchObject({
      dataset: {
        resultId: 'result-a',
        mountedResultId: 'result-a',
      },
    });

    act(() => {
      useResultWorkspaceStore.getState().openResult(descriptor('result-b'));
    });

    expect(container.querySelector('[data-testid="result-content"]')).toMatchObject({
      dataset: {
        resultId: 'result-b',
        mountedResultId: 'result-b',
      },
    });
  });
});
