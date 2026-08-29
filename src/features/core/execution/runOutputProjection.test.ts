import { describe, expect, it } from 'vitest';
import type { RunOutputChannelEvent, RunOutputEvent } from '@/shared/types/domain/runEvent';
import {
  RUN_OUTPUT_PROJECTION_MAX_ENTRIES,
  appendRunOutput,
  emptyRunOutputProjection,
} from './runOutputProjection';

const sourceGraphPath = 'functions/output.yssbi-function';
const sourceNodeId = '00000000-0000-0000-0000-000000000002';
const sourcePort = { kind: 'declared' as const, nodeId: sourceNodeId, portKey: 'message' };

function output(sequence: number, runId = '41'): RunOutputEvent {
  return {
    runId,
    sequence,
    stream: 'stdout',
    text: `output-${sequence}`,
    sourceGraphPath,
    sourceNodeId,
    sourcePort,
  };
}

describe('run output projection', () => {
  it('keeps one run in strict backend sequence order', () => {
    const first = appendRunOutput(emptyRunOutputProjection(), output(1));
    const second = appendRunOutput(first, output(2));

    expect(second).toMatchObject({ runId: '41', projectionDropped: false });
    expect(second.entries.map((entry) => entry.sequence)).toEqual([1, 2]);
    expect(appendRunOutput(second, output(2))).toBe(second);
    expect(appendRunOutput(second, output(3, '42'))).toBe(second);
  });

  it('marks a backend sequence gap explicitly while retaining later output', () => {
    const projection = appendRunOutput(emptyRunOutputProjection(), output(2));

    expect(projection.entries.map((entry) => entry.sequence)).toEqual([2]);
    expect(projection.projectionDropped).toBe(true);
  });

  it('stays bounded and exposes local projection loss explicitly', () => {
    let projection = emptyRunOutputProjection();
    for (let sequence = 1; sequence <= RUN_OUTPUT_PROJECTION_MAX_ENTRIES + 3; sequence += 1) {
      projection = appendRunOutput(projection, output(sequence));
    }
    const status: RunOutputChannelEvent = {
      runId: '41',
      sequence: RUN_OUTPUT_PROJECTION_MAX_ENTRIES + 4,
      stream: 'stdout',
      status: 'dropped',
      sourceGraphPath,
      sourceNodeId,
      sourcePort,
    };
    projection = appendRunOutput(projection, status);

    expect(projection.entries).toHaveLength(RUN_OUTPUT_PROJECTION_MAX_ENTRIES);
    expect(projection.entries[0]?.sequence).toBe(5);
    expect(projection.entries[projection.entries.length - 1]).toEqual(status);
    expect(projection.projectionDropped).toBe(true);
  });
});
