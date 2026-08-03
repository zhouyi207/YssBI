import { describe, expect, it } from 'vitest';
import { DEFAULT_EXECUTION_DEMAND, type ExecutionDemandDto } from './executionDemand';
import type { RunEventKind } from './runEvent';

const declaredOutput = {
  graphPath: 'events/Main.yssbi-event',
  port: {
    kind: 'declared',
    nodeId: '00000000-0000-0000-0000-000000000001',
    portKey: 'result',
  },
} as const;

const instanceOutput = {
  graphPath: 'events/Main.yssbi-event',
  port: {
    kind: 'instance',
    nodeId: '00000000-0000-0000-0000-000000000001',
    templateKey: 'results',
    instanceId: '00000000-0000-0000-0000-000000000002',
  },
} as const;

describe('ExecutionDemandDto', () => {
  it('freezes default, declared, instance, empty, and duplicate-order wire shapes', () => {
    const outputs: ExecutionDemandDto = {
      type: 'outputs',
      outputs: [declaredOutput, instanceOutput, declaredOutput],
      includeDefaultResults: true,
    };
    const empty: ExecutionDemandDto = {
      type: 'outputs',
      outputs: [],
      includeDefaultResults: false,
    };

    expect(DEFAULT_EXECUTION_DEMAND).toEqual({ type: 'default' });
    expect(outputs.outputs).toEqual([declaredOutput, instanceOutput, declaredOutput]);
    expect(empty.outputs).toEqual([]);
  });

  it('freezes stable outputReady identity without compiler-local fields', () => {
    const event: RunEventKind = {
      type: 'outputReady',
      output: declaredOutput,
      sourceId: '42',
    };

    expect(event).toEqual({ type: 'outputReady', output: declaredOutput, sourceId: '42' });
  });
});

// These compile-time assertions are checked by `pnpm typecheck`.
// @ts-expect-error outputs demand requires outputs
const missingOutputs: ExecutionDemandDto = {
  type: 'outputs',
  includeDefaultResults: false,
};

// @ts-expect-error outputs demand requires includeDefaultResults
const missingIncludeDefaults: ExecutionDemandDto = { type: 'outputs', outputs: [] };

// @ts-expect-error demand tags are closed
const invalidTag: ExecutionDemandDto = { type: 'unknown' };

// @ts-expect-error default demand has no extra fields
const extraDefaultField: ExecutionDemandDto = { type: 'default', outputs: [] };

const valueIndexDemand: ExecutionDemandDto = {
  type: 'outputs',
  outputs: [
    {
      graphPath: 'events/Main.yssbi-event',
      port: declaredOutput.port,
      // @ts-expect-error output identities cannot carry valueIndex
      valueIndex: 1,
    },
  ],
  includeDefaultResults: false,
};

const operationIndexDemand: ExecutionDemandDto = {
  type: 'outputs',
  outputs: [
    {
      graphPath: 'events/Main.yssbi-event',
      port: declaredOutput.port,
      // @ts-expect-error output identities cannot carry operationIndex
      operationIndex: 1,
    },
  ],
  includeDefaultResults: false,
};

void [
  missingOutputs,
  missingIncludeDefaults,
  invalidTag,
  extraDefaultField,
  valueIndexDemand,
  operationIndexDemand,
];
