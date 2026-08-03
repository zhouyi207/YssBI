import { describe, expect, it } from 'vitest';
import type { RunCorrelationDto, RunEvent } from './runEvent';

function correlation(selectionDigest: string): RunCorrelationDto {
  return {
    projectSessionId: 'project-session-1',
    graphPath: 'events/Main.yssbi-event',
    graphRevision: '7',
    registryFingerprint: 'registry-1',
    resourceVersions: {},
    compileId: '9',
    selectionDigest,
    runId: '41',
    nodeId: null,
    nodeTypeId: null,
    parentCall: null,
  };
}

describe('canonical run correlation contract', () => {
  it('keeps selection digest separate from the full compile identity', () => {
    const first: RunEvent = {
      correlation: correlation('demand-selection-a'),
      basis: {
        graphRevision: '7',
        registryFingerprint: 'registry-1',
        resourceVersions: {},
      },
      kind: { type: 'runStarted' },
    };
    const second: RunEvent = {
      ...first,
      correlation: correlation('demand-selection-b'),
    };

    expect(first.correlation.compileId).toBe(second.correlation.compileId);
    expect(first.correlation.selectionDigest).not.toBe(second.correlation.selectionDigest);
    expect(Object.keys(first.correlation)).toEqual([
      'projectSessionId',
      'graphPath',
      'graphRevision',
      'registryFingerprint',
      'resourceVersions',
      'compileId',
      'selectionDigest',
      'runId',
      'nodeId',
      'nodeTypeId',
      'parentCall',
    ]);
  });
});
