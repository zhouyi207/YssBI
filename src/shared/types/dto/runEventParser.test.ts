import { describe, expect, it } from 'vitest';
import executionWire from '@/tests/fixtures/node-system-contracts/execution-wire.json';
import { EXECUTION_DEMAND_TYPES } from './executionDemand';
import { RUN_EVENT_KIND_TYPES } from './runEvent';
import {
  parseExecutionChannelEvent,
  parseExecutionDemandDto,
  parseRunEvent,
} from './runEventParser';

function clone(value: unknown): unknown {
  return structuredClone(value);
}

function record(value: unknown): Record<string, unknown> {
  return value as Record<string, unknown>;
}

describe('execution wire parsers', () => {
  it('parses every Rust-generated execution demand variant', () => {
    expect(executionWire.demands.map(parseExecutionDemandDto)).toEqual(executionWire.demands);
    expect(executionWire.demands.map((demand) => demand.type))
      .toEqual(Object.keys(EXECUTION_DEMAND_TYPES));
    expect(() => parseExecutionDemandDto({
      type: 'outputs',
      outputs: [],
      includeDefaultResults: false,
    })).not.toThrow();
  });

  it.each(executionWire.demands)('rejects extra keys on demand $type', (valid) => {
    expect(() => parseExecutionDemandDto({ ...valid, extra: true })).toThrow();
  });

  it('strictly validates output references and both port-address variants', () => {
    const outputs = executionWire.demands.find((demand) => demand.type === 'outputs');
    if (!outputs?.outputs) throw new Error('missing outputs fixture');
    expect(outputs.outputs.map((output) => output.port.kind)).toEqual(['declared', 'instance']);

    const extraOutput = clone(outputs);
    Object.assign(
      (record(extraOutput).outputs as Array<Record<string, unknown>>)[0],
      { extra: true },
    );
    expect(() => parseExecutionDemandDto(extraOutput)).toThrow();

    const extraPort = clone(outputs);
    const firstOutput = (record(extraPort).outputs as Array<Record<string, unknown>>)[0];
    Object.assign(firstOutput.port as object, { extra: true });
    expect(() => parseExecutionDemandDto(extraPort)).toThrow();
  });

  it.each([
    '',
    'not-a-resource',
    'events/contract.yssbi-function',
    'functions/contract.yssbi-event',
    'events//contract.yssbi-event',
    'events/../contract.yssbi-event',
  ])('rejects malformed graph output path %j', (graphPath) => {
    const outputs = clone(executionWire.demands.find((demand) => demand.type === 'outputs'));
    const firstOutput = (record(outputs).outputs as Array<Record<string, unknown>>)[0];
    firstOutput.graphPath = graphPath;

    expect(() => parseExecutionDemandDto(outputs)).toThrow('graph output reference');
  });

  it.each([
    'events/folder/sub-folder/Main.v2.yssbi-event',
    'functions/library/math/Calculate.yssbi-function',
    'events/Sales Report 中文.yssbi-event',
    'functions/销售 预测.yssbi-function',
  ])('accepts opaque execution graph path %j', (graphPath) => {
    const outputs = clone(executionWire.demands.find((demand) => demand.type === 'outputs'));
    const firstOutput = (record(outputs).outputs as Array<Record<string, unknown>>)[0];
    firstOutput.graphPath = graphPath;
    expect(() => parseExecutionDemandDto(outputs)).not.toThrow();

    const event = executionWire.runEvents[0];
    expect(() => parseRunEvent({
      ...event,
      run: { ...event.run, graphPath },
    })).not.toThrow();
  });

  it('requires UUID-backed declared and instance port identities', () => {
    const outputs = clone(executionWire.demands.find((demand) => demand.type === 'outputs'));
    const references = record(outputs).outputs as Array<Record<string, unknown>>;

    (references[0].port as Record<string, unknown>).nodeId = 'not-a-uuid';
    expect(() => parseExecutionDemandDto(outputs)).toThrow('graph output reference');

    (references[0].port as Record<string, unknown>).nodeId =
      '00000000-0000-0000-0000-000000000002';
    (references[1].port as Record<string, unknown>).nodeId = 'not-a-uuid';
    expect(() => parseExecutionDemandDto(outputs)).toThrow('graph output reference');

    (references[1].port as Record<string, unknown>).nodeId =
      '00000000-0000-0000-0000-000000000002';
    (references[1].port as Record<string, unknown>).instanceId = 'not-a-uuid';
    expect(() => parseExecutionDemandDto(outputs)).toThrow('graph output reference');
  });

  it('parses the exact minimal Rust-generated run-event inventory', () => {
    const valid = executionWire.runEvents[0];

    expect(executionWire.runEvents.map(parseRunEvent)).toEqual(executionWire.runEvents);
    expect([...new Set(executionWire.runEvents.map((event) => event.kind.type))])
      .toEqual(Object.keys(RUN_EVENT_KIND_TYPES));

    expect(() => parseRunEvent({
      correlation: {},
      basis: {},
      kind: { type: 'runStarted' },
    })).toThrow('Invalid run event');

    expect(() => parseRunEvent({
      ...valid,
      run: { ...valid.run, runId: null },
    })).toThrow('Invalid graph run identity');
  });

  it('parses run output separately from lifecycle events', () => {
    expect(executionWire.runOutputEvents.map(parseExecutionChannelEvent))
      .toEqual(executionWire.runOutputEvents);
    const output = {
      runId: '41',
      sequence: 1,
      stream: 'stdout',
      text: 'user-visible value',
      sourceGraphPath: 'functions/output.yssbi-function',
      sourceNodeId: '00000000-0000-0000-0000-000000000002',
    } as const;
    const truncated = {
      runId: '41',
      sequence: 2,
      stream: 'stdout',
      status: 'truncated',
      sourceGraphPath: 'functions/output.yssbi-function',
      sourceNodeId: '00000000-0000-0000-0000-000000000002',
    } as const;

    expect(parseExecutionChannelEvent(output)).toEqual(output);
    expect(parseExecutionChannelEvent(truncated)).toEqual(truncated);
    expect(() => parseRunEvent(output)).toThrow('run event');
    expect(() => parseExecutionChannelEvent({ ...output, sequence: -1 })).toThrow();
    expect(() => parseExecutionChannelEvent({ ...output, runId: '0' })).toThrow();
    expect(() => parseExecutionChannelEvent({ ...output, sourceGraphPath: 'not-a-resource' })).toThrow();
    expect(() => parseExecutionChannelEvent({ ...output, stream: 'diagnostic' })).toThrow();
    expect(() => parseExecutionChannelEvent({ ...output, extra: true })).toThrow();
    expect(() => parseExecutionChannelEvent({ ...truncated, status: 'warning' })).toThrow();
  });

  it.each([
    'operationStarted',
    'operationCompleted',
    'operationErrored',
    'resultGroupChanged',
    'outputResultChanged',
  ])('rejects removed run-event variant %s', (type) => {
    const valid = executionWire.runEvents[0];
    expect(() => parseRunEvent({
      ...valid,
      kind: { type },
    })).toThrow('Invalid run event kind variant');
  });

  it.each(executionWire.runEvents)('rejects extra keys on RunEvent $kind.type', (valid) => {
    expect(() => parseRunEvent({ ...valid, extra: true })).toThrow();
    expect(() => parseRunEvent({ ...valid, run: { ...valid.run, extra: true } })).toThrow();
    expect(() => parseRunEvent({ ...valid, kind: { ...valid.kind, extra: true } })).toThrow();
  });

  it('requires an exact pinPreviewResultReady generation wire', () => {
    const preview = executionWire.runEvents.find(
      (event) => event.kind.type === 'pinPreviewResultReady',
    );
    if (!preview) throw new Error('missing pin preview result fixture');

    const missingGeneration = clone(preview);
    delete record(record(missingGeneration).kind).generation;
    expect(() => parseRunEvent(missingGeneration)).toThrow();
    expect(parseRunEvent(preview)).toEqual(preview);
  });

  it('strictly parses typed deadline phases and rejects malformed timeout wire', () => {
    const valid = executionWire.runEvents[0];
    const deadline = {
      ...valid,
      kind: { type: 'runErrored', code: 'deadlineExceeded', phase: 'queueWait' },
    };

    expect(parseRunEvent(deadline)).toEqual(deadline);
    for (const kind of [
      { type: 'runErrored', code: 'deadlineExceeded' },
      { type: 'runErrored', code: 'deadlineExceeded', phase: null },
      { type: 'runErrored', code: 'deadlineExceeded', phase: 'unknown' },
      { type: 'runErrored', code: 'kernelFailed', phase: 'kernel' },
      { type: 'runErrored', code: 'kernelFailed' },
    ]) {
      expect(() => parseRunEvent({ ...valid, kind })).toThrow();
    }
  });

  it('rejects unknown variants and malformed graph run identities', () => {
    const valid = executionWire.runEvents[0];
    expect(() => parseRunEvent({ ...valid, kind: { type: 'unknown' } })).toThrow();
    expect(() => parseRunEvent({
      ...valid,
      run: { ...valid.run, runId: '01' },
    })).toThrow('graph run identity');
    expect(() => parseRunEvent({
      ...valid,
      run: { ...valid.run, projectSessionId: '' },
    })).toThrow('graph run identity');
  });

  it.each([
    'not-a-resource',
    'events/contract.yssbi-function',
    'functions/contract.yssbi-event',
    'events//contract.yssbi-event',
    'events/../contract.yssbi-event',
  ])('rejects malformed graph run path %j', (graphPath) => {
    const valid = executionWire.runEvents[0];
    expect(() => parseRunEvent({
      ...valid,
      run: { ...valid.run, graphPath },
    })).toThrow('graph run identity');
  });

  it('rejects malformed pinPreviewResultReady graph and port identities', () => {
    const preview = executionWire.runEvents.find(
      (event) => event.kind.type === 'pinPreviewResultReady',
    );
    if (!preview) throw new Error('missing pinPreviewResultReady fixture');

    const malformedPath = clone(preview);
    (record(record(malformedPath).kind).output as Record<string, unknown>).graphPath =
      'functions/contract.yssbi-event';
    expect(() => parseRunEvent(malformedPath)).toThrow('graph output reference');

    const malformedPort = clone(preview);
    const output = record(record(malformedPort).kind).output as Record<string, unknown>;
    (output.port as Record<string, unknown>).nodeId = 'not-a-uuid';
    expect(() => parseRunEvent(malformedPort)).toThrow('graph output reference');
  });

  it('bounds safe integer preview generations', () => {
    const preview = executionWire.runEvents.find(
      (event) => event.kind.type === 'pinPreviewResultReady',
    );
    if (!preview) throw new Error('missing preview event fixture');

    expect(() => parseRunEvent({
      ...preview,
      kind: { ...preview.kind, generation: Number.MAX_SAFE_INTEGER },
    })).not.toThrow();
    expect(() => parseRunEvent({
      ...preview,
      kind: { ...preview.kind, generation: Number.MAX_SAFE_INTEGER + 1 },
    })).toThrow();
  });
});
