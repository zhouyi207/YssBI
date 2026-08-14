import { describe, expect, it } from 'vitest';
import executionWire from '@/tests/fixtures/node-system-contracts/execution-wire.json';
import { EXECUTION_DEMAND_TYPES } from './executionDemand';
import { RUN_EVENT_KIND_TYPES } from './runEvent';
import {
  parseExecuteGraphResultDto,
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
      correlation: { ...event.correlation, graphPath },
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

  it('parses every Rust-generated RunEventKindDto variant', () => {
    expect(executionWire.runEvents.map(parseRunEvent)).toEqual(executionWire.runEvents);
    expect([...new Set(executionWire.runEvents.map((event) => event.kind.type))])
      .toEqual(Object.keys(RUN_EVENT_KIND_TYPES));
  });

  it('requires exact decimal attempt identity on every operation event', () => {
    const operations = executionWire.runEvents.filter(
      (event) => event.kind.type === 'operationStarted'
        || event.kind.type === 'operationCompleted'
        || event.kind.type === 'operationErrored',
    );
    for (const operation of operations) {
      expect(operation.kind).toHaveProperty('attemptId');
      const missing = clone(operation);
      delete record(record(missing).kind).attemptId;
      expect(() => parseRunEvent(missing)).toThrow();
      const wrong = clone(operation);
      record(record(wrong).kind).attemptId = 1;
      expect(() => parseRunEvent(wrong)).toThrow();
    }
  });

  it.each(executionWire.runEvents)('rejects extra keys on RunEvent $kind.type', (valid) => {
    expect(() => parseRunEvent({ ...valid, extra: true })).toThrow();
    expect(() => parseRunEvent({ ...valid, kind: { ...valid.kind, extra: true } })).toThrow();
  });

  it('requires exact outputResultChanged generation wire', () => {
    const outputResultChanged = executionWire.runEvents.find((event) => event.kind.type === 'outputResultChanged');
    if (!outputResultChanged) throw new Error('missing output publication fixture');

    const missingGeneration = clone(outputResultChanged);
    delete record(record(missingGeneration).kind).generation;
    expect(() => parseRunEvent(missingGeneration)).toThrow();
    expect(parseRunEvent(outputResultChanged)).toEqual(outputResultChanged);
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

  it('rejects unknown and malformed run event variants', () => {
    const valid = executionWire.runEvents[0];
    expect(() => parseRunEvent({ ...valid, kind: { type: 'unknown' } })).toThrow();
    expect(() => parseRunEvent({
      ...valid,
      correlation: { ...valid.correlation, compileId: 9_007_199_254_740_993 },
    })).toThrow();
    expect(() => parseRunEvent({
      ...valid,
      correlation: { ...valid.correlation, runId: '01' },
    })).toThrow();
  });

  it.each([
    'not-a-resource',
    'events/contract.yssbi-function',
    'functions/contract.yssbi-event',
    'events//contract.yssbi-event',
    'events/../contract.yssbi-event',
  ])('rejects malformed run correlation graph path %j', (graphPath) => {
    const valid = executionWire.runEvents[0];
    expect(() => parseRunEvent({
      ...valid,
      correlation: { ...valid.correlation, graphPath },
    })).toThrow('run correlation');
  });

  it('requires a UUID correlation nodeId while keeping nodeTypeId opaque', () => {
    const valid = executionWire.runEvents[0];
    expect(() => parseRunEvent({
      ...valid,
      correlation: { ...valid.correlation, nodeId: 'not-a-uuid' },
    })).toThrow('run correlation');
    expect(() => parseRunEvent({
      ...valid,
      correlation: {
        ...valid.correlation,
        nodeId: '00000000-0000-0000-0000-000000000002',
        nodeTypeId: 'opaque registry identifier/with spaces',
      },
    })).not.toThrow();
  });

  it('rejects malformed OutputResultChanged graph and port identities', () => {
    const outputResultChanged = executionWire.runEvents.find((event) => event.kind.type === 'outputResultChanged');
    if (!outputResultChanged) throw new Error('missing outputResultChanged fixture');

    const malformedPath = clone(outputResultChanged);
    (record(record(malformedPath).kind).output as Record<string, unknown>).graphPath =
      'functions/contract.yssbi-event';
    expect(() => parseRunEvent(malformedPath)).toThrow('graph output reference');

    const malformedPort = clone(outputResultChanged);
    const output = record(record(malformedPort).kind).output as Record<string, unknown>;
    (output.port as Record<string, unknown>).nodeId = 'not-a-uuid';
    expect(() => parseRunEvent(malformedPort)).toThrow('graph output reference');
  });

  it('bounds u32 operation indexes and safe integer preview generations', () => {
    const operation = executionWire.runEvents.find(
      (event) => event.kind.type === 'operationStarted',
    );
    const output = executionWire.runEvents.find((event) => event.kind.type === 'outputResultChanged');
    if (!operation || !output) throw new Error('missing indexed event fixtures');

    expect(() => parseRunEvent({
      ...operation,
      kind: { ...operation.kind, operationIndex: 4_294_967_295 },
    })).not.toThrow();
    expect(() => parseRunEvent({
      ...operation,
      kind: { ...operation.kind, operationIndex: 4_294_967_296 },
    })).toThrow();
    expect(() => parseRunEvent({
      ...output,
      kind: { ...output.kind, generation: Number.MAX_SAFE_INTEGER },
    })).not.toThrow();
    expect(() => parseRunEvent({
      ...output,
      kind: { ...output.kind, generation: Number.MAX_SAFE_INTEGER + 1 },
    })).toThrow();
  });

  it('requires lowercase 64-hex Registry fingerprints in correlation and basis', () => {
    const valid = executionWire.runEvents[0];
    expect(() => parseRunEvent({
      ...valid,
      correlation: {
        ...valid.correlation,
        registryFingerprint: valid.correlation.registryFingerprint.toUpperCase(),
      },
    })).toThrow();
    expect(() => parseRunEvent({
      ...valid,
      basis: {
        ...valid.basis,
        registryFingerprint: valid.basis.registryFingerprint.slice(1),
      },
    })).toThrow();
  });

  it('parses only an exact execute graph result with an opaque decimal string ID', () => {
    expect(parseExecuteGraphResultDto(executionWire.executeGraphResult))
      .toEqual(executionWire.executeGraphResult);
    expect(() => parseExecuteGraphResultDto({ runId: 41 })).toThrow();
    expect(() => parseExecuteGraphResultDto({ runId: '41', extra: true })).toThrow();
    expect(() => parseExecuteGraphResultDto({ runId: '01' })).toThrow();
  });
});
