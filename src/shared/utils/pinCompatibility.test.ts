import { describe, it, expect } from 'vitest';
import type { PinDirection } from '@/shared/types/domain/pin';
import type { PinSlot } from '@/shared/types/domain/node';
import { dataTypeDisplay, type DataType } from '@/shared/types/domain/dataType';
import type { TypeSystemSnapshot } from '@/shared/types/domain/typeSystem';
import {
  buildPinDataType,
  pinAcceptsType,
  isPinCompatible,
  resolveConnectionCompatibility,
  findAutoConnectPinIndex,
  getDataTypeCompatibility,
  getPinCompatibility,
  type ConnectionCandidatePin,
} from './pinCompatibility';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';

const FLOAT64: DataType = { kind: 'Float64' };
const STRING: DataType = { kind: 'String' };
const INT64: DataType = { kind: 'Int64' };
const SERIES_FLOAT64: DataType = { kind: 'DataSeries', inner: { kind: 'Float64' } };
const MODEL: DataType = { kind: 'Struct', inner: 'Model' };
const OLS_MODEL: DataType = { kind: 'Struct', inner: 'OLSModel' };
const OLS_RESULT: DataType = { kind: 'Struct', inner: 'OLSResult' };

const TYPE_SYSTEM: TypeSystemSnapshot = {
  structTypes: {
    Model: { key: 'Model', parents: [], category: 'model' },
    OLSModel: { key: 'OLSModel', parents: ['Model'], category: 'model' },
    OLSResult: { key: 'OLSResult', parents: [], category: 'result' },
  },
};

function pin(
  partial: Partial<ConnectionCandidatePin> & { direction: PinDirection },
): ConnectionCandidatePin {
  return {
    id: partial.id ?? 'p1',
    nodeId: partial.nodeId ?? 'n1',
    name: partial.name ?? 'pin',
    type: partial.type ?? 'object',
    ...partial,
  } as ConnectionCandidatePin;
}

describe('dataTypeDisplay Number alias', () => {
  it('uses Number only for the exact scalar numeric union', () => {
    expect(dataTypeDisplay({
      kind: 'OneOf',
      inner: [{ kind: 'Float64' }, { kind: 'Int64' }],
    })).toBe('Number');
    expect(dataTypeDisplay({
      kind: 'OneOf',
      inner: [{ kind: 'Float64' }, { kind: 'String' }],
    })).toBe('Float64 | String');
  });

  it('uses DataSeries<Number> only for the exact outer numeric series union', () => {
    expect(dataTypeDisplay({
      kind: 'OneOf',
      inner: [
        { kind: 'DataSeries', inner: { kind: 'Int64' } },
        { kind: 'DataSeries', inner: { kind: 'Float64' } },
      ],
    })).toBe('DataSeries<Number>');
    expect(dataTypeDisplay({
      kind: 'DataSeries',
      inner: { kind: 'OneOf', inner: [{ kind: 'Int64' }, { kind: 'Float64' }] },
    })).not.toBe('DataSeries<Number>');
  });
});

describe('getDataTypeCompatibility', () => {
  it('requires every source union member to be assignable', () => {
    expect(getDataTypeCompatibility(
      { kind: 'OneOf', inner: [{ kind: 'Int64' }, { kind: 'String' }] },
      { kind: 'Int64' },
    )).toBe('incompatible');
  });

  it('accepts when every source union member is assignable', () => {
    expect(getDataTypeCompatibility(
      { kind: 'OneOf', inner: [{ kind: 'Int64' }, { kind: 'Float64' }] },
      { kind: 'OneOf', inner: [{ kind: 'Float64' }, { kind: 'Int64' }] },
    )).toBe('compatible');
  });

  it('returns indeterminate when either projected type is missing', () => {
    expect(getDataTypeCompatibility(null, { kind: 'Float64' })).toBe('indeterminate');
    expect(getDataTypeCompatibility({ kind: 'Float64' }, undefined)).toBe('indeterminate');
  });

  it('accepts homogeneous numeric series into DataSeries Number union', () => {
    const target = { kind: 'OneOf', inner: [
      { kind: 'DataSeries', inner: { kind: 'Int64' } },
      { kind: 'DataSeries', inner: { kind: 'Float64' } },
    ] } satisfies DataType;

    expect(getDataTypeCompatibility(SERIES_FLOAT64, target)).toBe('compatible');
  });

  it('does not treat Any as a wildcard', () => {
    expect(getDataTypeCompatibility({ kind: 'Any' }, FLOAT64)).toBe('incompatible');
    expect(getDataTypeCompatibility(FLOAT64, { kind: 'Any' })).toBe('incompatible');
  });
});

describe('getPinCompatibility', () => {
  it('returns indeterminate for unresolved projected pins', () => {
    const output = pin({
      id: 'output',
      nodeId: 'source',
      direction: 'output',
      kind: 'data',
      resolvedType: { display: 'Unknown', resolved: false, dataType: null },
    });
    const input = pin({
      id: 'input',
      nodeId: 'target',
      direction: 'input',
      kind: 'data',
      resolvedType: { display: 'core.float64', resolved: true, dataType: FLOAT64 },
      dataType: FLOAT64,
    });

    expect(getPinCompatibility(output, input)).toBe('indeterminate');
  });
});

describe('buildPinDataType', () => {
  it('requires structured dataType for data pins', () => {
    const p = pin({
      direction: 'output',
      type: 'object',
      dataType: SERIES_FLOAT64,
    });
    expect(buildPinDataType(p)).toEqual(SERIES_FLOAT64);
  });

  it('throws for data pins without structured dataType', () => {
    const p = pin({ direction: 'output', type: 'object' });
    expect(() => buildPinDataType(p)).toThrow('missing structured dataType');
  });

  it('keeps exec pins outside the data type system', () => {
    const p = pin({ direction: 'output', type: 'exec' });
    expect(buildPinDataType(p)).toEqual({ kind: 'Any' });
  });
});

describe('pinAcceptsType - variable Set recommendations (dragging an output)', () => {
  const draggedSeriesOutput = pin({ direction: 'output', dataType: SERIES_FLOAT64 });

  it('recommends a DataSeries<Float64> variable', () => {
    expect(pinAcceptsType(draggedSeriesOutput, SERIES_FLOAT64)).toBe(true);
  });

  it('does not recommend a scalar Float64 variable for a DataSeries output', () => {
    expect(pinAcceptsType(draggedSeriesOutput, FLOAT64)).toBe(false);
  });
});

describe('pinAcceptsType - variable Get recommendations (dragging an input)', () => {
  const draggedFloatInput = pin({ direction: 'input', dataType: FLOAT64 });

  it('recommends a Float64 variable', () => {
    expect(pinAcceptsType(draggedFloatInput, FLOAT64)).toBe(true);
  });

  it('does not recommend a String variable', () => {
    expect(pinAcceptsType(draggedFloatInput, STRING)).toBe(false);
  });
});

describe('pinAcceptsType - OneOf pin converges to compatible members only', () => {
  const oneOf: DataType = { kind: 'OneOf', inner: [FLOAT64, SERIES_FLOAT64] };
  const draggedOneOfInput = pin({ direction: 'input', dataType: oneOf });

  it('matches members of the OneOf set', () => {
    expect(pinAcceptsType(draggedOneOfInput, FLOAT64)).toBe(true);
    expect(pinAcceptsType(draggedOneOfInput, SERIES_FLOAT64)).toBe(true);
  });

  it('rejects types outside the OneOf set', () => {
    expect(pinAcceptsType(draggedOneOfInput, STRING)).toBe(false);
    expect(pinAcceptsType(draggedOneOfInput, INT64)).toBe(false);
  });
});

describe('pinAcceptsType - function IO filtered by precise type', () => {
  it('a DataSeries<Float64> function input accepts only a matching output', () => {
    const draggedSeriesOutput = pin({ direction: 'output', dataType: SERIES_FLOAT64 });
    const funcInput = pin({ direction: 'input', nodeId: 'fn', dataType: SERIES_FLOAT64 });
    const funcScalarInput = pin({ direction: 'input', nodeId: 'fn', dataType: FLOAT64 });
    expect(pinAcceptsType(draggedSeriesOutput, buildPinDataType(funcInput))).toBe(true);
    expect(pinAcceptsType(draggedSeriesOutput, buildPinDataType(funcScalarInput))).toBe(false);
  });
});

describe('pinAcceptsType - Struct family matching', () => {
  const draggedModelOutput = pin({ direction: 'output', dataType: OLS_MODEL });

  it('allows a concrete model output to connect to a Model family input', () => {
    expect(pinAcceptsType(draggedModelOutput, MODEL, TYPE_SYSTEM)).toBe(true);
  });

  it('does not allow unrelated Struct outputs into a Model family input', () => {
    const draggedResultOutput = pin({ direction: 'output', dataType: OLS_RESULT });
    expect(pinAcceptsType(draggedResultOutput, MODEL, TYPE_SYSTEM)).toBe(false);
  });
});

describe('findAutoConnectPinIndex via Rust-shaped editor projection', () => {
  it('connects a data output to projected input pin slots without rebuilding a function signature', () => {
    const fixture = makeEditorProjectionFixture({
      graphPath: 'events/Main.yssbi-event',
      nodeId: 'projected-target',
      nodeTypeId: 'tests.projected-target',
      title: 'Projected target',
    });
    const projectedInput = fixture.projection.nodes[0].ports.find(
      (port) => port.direction === 'input',
    );
    if (!projectedInput) throw new Error('expected projected input');
    const pinSlots: PinSlot[] = [{
      slotKind: 'fixed',
      pin: {
        name: projectedInput.display.label,
        direction: projectedInput.direction,
        kind: 'Data',
        role: { Data: { Custom: projectedInput.templateKey } },
        dataType: { Concrete: FLOAT64 },
        optional: false,
        metaData: { showWidget: false, widgetType: null, isDynamic: false },
      },
    }];
    const draggedOutput = pin({ direction: 'output', dataType: FLOAT64 });

    expect(findAutoConnectPinIndex(pinSlots, draggedOutput)).toBe(0);
  });
});

describe('isPinCompatible', () => {
  it('matches output -> input of the same structured type', () => {
    const out = pin({ id: 'o', nodeId: 'a', direction: 'output', dataType: SERIES_FLOAT64 });
    const inSeries = pin({ id: 'i', nodeId: 'b', direction: 'input', dataType: SERIES_FLOAT64 });
    const inScalar = pin({ id: 'i2', nodeId: 'b', direction: 'input', dataType: FLOAT64 });
    expect(isPinCompatible(inSeries, out)).toBe(true);
    expect(isPinCompatible(inScalar, out)).toBe(false);
  });
});

describe('resolveConnectionCompatibility', () => {
  const appendCapability = {
    current: 0,
    maximum: 1,
    ordered: false,
    canAppend: true,
    canReplace: false,
    canMove: false,
  };

  const output = pin({
    id: 'output',
    nodeId: 'source',
    direction: 'output',
    kind: 'data',
    dataType: FLOAT64,
    connections: appendCapability,
  });
  const input = pin({
    id: 'input',
    nodeId: 'target',
    direction: 'input',
    kind: 'data',
    dataType: FLOAT64,
    connections: appendCapability,
  });

  it('returns append for compatible append-capable endpoints', () => {
    expect(resolveConnectionCompatibility(output, input)).toEqual({ kind: 'append' });
  });

  it('returns replace without displaced connection IDs', () => {
    const replaceable = pin({
      ...input,
      connections: {
        ...appendCapability,
        current: 1,
        canAppend: false,
        canReplace: true,
      },
    });

    expect(resolveConnectionCompatibility(output, replaceable)).toEqual({ kind: 'replace' });
  });

  it.each([
    ['samePort', output, output],
    ['sameNode', output, pin({ ...input, nodeId: output.nodeId })],
    ['directionMismatch', output, pin({ ...input, direction: 'output' })],
    ['kindMismatch', output, pin({ ...input, type: 'exec', kind: 'control', dataType: undefined })],
    ['typeMismatch', output, pin({ ...input, dataType: STRING })],
    ['orphan', output, pin({ ...input, orphan: true })],
    ['capacityReached', output, pin({
      ...input,
      connections: {
        ...appendCapability,
        current: 1,
        canAppend: false,
        canReplace: false,
      },
    })],
  ] as const)('returns the %s invalid reason', (reason, source, target) => {
    expect(resolveConnectionCompatibility(source, target)).toEqual({ kind: 'invalid', reason });
  });

  it.each(['control', 'effect'] as const)('checks %s capability before succeeding', (kind) => {
    const source = pin({
      ...output,
      type: 'exec',
      kind,
      dataType: undefined,
    });
    const fullTarget = pin({
      ...input,
      type: 'exec',
      kind,
      dataType: undefined,
      connections: {
        ...appendCapability,
        current: 1,
        canAppend: false,
        canReplace: false,
      },
    });

    expect(resolveConnectionCompatibility(source, fullTarget)).toEqual({
      kind: 'invalid',
      reason: 'capacityReached',
    });
  });

  it('does not allow control and effect exec pins to interconnect', () => {
    const control = pin({ ...output, type: 'exec', kind: 'control', dataType: undefined });
    const effect = pin({ ...input, type: 'exec', kind: 'effect', dataType: undefined });

    expect(resolveConnectionCompatibility(control, effect)).toEqual({
      kind: 'invalid',
      reason: 'kindMismatch',
    });
  });

  it('preserves structured data type compatibility and argument-order independence', () => {
    const modelOutput = pin({ id: 'modelOut', nodeId: 'ols', direction: 'output', dataType: OLS_MODEL });
    const modelInput = pin({ id: 'modelIn', nodeId: 'predict', direction: 'input', dataType: MODEL });
    const resultInput = pin({ id: 'resultIn', nodeId: 'consumer', direction: 'input', dataType: OLS_RESULT });

    expect(resolveConnectionCompatibility(modelOutput, modelInput, TYPE_SYSTEM)).toEqual({ kind: 'append' });
    expect(resolveConnectionCompatibility(modelInput, modelOutput, TYPE_SYSTEM)).toEqual({ kind: 'append' });
    expect(resolveConnectionCompatibility(modelOutput, resultInput, TYPE_SYSTEM)).toEqual({
      kind: 'invalid',
      reason: 'typeMismatch',
    });
  });
});
