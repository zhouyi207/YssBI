import { describe, expect, it } from 'vitest';

import type { GraphOutputRefDto, ResultDescriptor } from '@/shared/types/dto/result';
import { resultPanelKey } from './resultPanelKey';

const outputRef: GraphOutputRefDto = {
  graphPath: 'events/Main.yssbi-event',
  port: { kind: 'declared', nodeId: 'node-a', portKey: 'result' },
};

const otherOutputRef: GraphOutputRefDto = {
  graphPath: 'events/Main.yssbi-event',
  port: { kind: 'instance', nodeId: 'node-b', templateKey: 'value', instanceId: '2' },
};

function descriptor(
  resultId: string,
  output: GraphOutputRefDto | null,
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
    title: `Result ${resultId}`,
  };
}

describe('resultPanelKey', () => {
  it('reuses one logical key for newer payloads from the same output', () => {
    expect(resultPanelKey(descriptor('result-1', outputRef)))
      .toBe(resultPanelKey(descriptor('result-2', outputRef)));
  });

  it('keeps unrelated and output-less results distinct', () => {
    expect(resultPanelKey(descriptor('result-1', outputRef)))
      .not.toBe(resultPanelKey(descriptor('result-1', otherOutputRef)));
    expect(resultPanelKey(descriptor('a', null)))
      .not.toBe(resultPanelKey(descriptor('b', null)));
    expect(resultPanelKey(descriptor('a:b', null))).toBe('result:3:a:b');
  });
});
