import { describe, expect, it } from 'vitest';
import type { ExecutionDemandDto } from './executionDemand';
import { parseExecutionDemandDto } from './runEventParser';

const declaredOutput = {
  graphPath: 'events/Main.yssbi-event',
  port: {
    kind: 'declared',
    nodeId: '00000000-0000-0000-0000-000000000001',
    portKey: 'result',
  },
} as const;

describe('ExecutionDemandDto', () => {
  it('strictly parses an independent pin preview demand with generation', () => {
    const preview = {
      type: 'pinPreview',
      output: declaredOutput,
      generation: 17,
    };

    expect(parseExecutionDemandDto(preview)).toEqual(preview);
    expect(() => parseExecutionDemandDto({ ...preview, includeDefaultResults: false })).toThrow();
  });
});

// These compile-time assertions are checked by `pnpm check:ts`.
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
