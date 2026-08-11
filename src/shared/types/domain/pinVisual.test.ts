import { describe, expect, it } from 'vitest';
import { resolvePinVisualSpec } from './pinVisual';

describe('resolvePinVisualSpec', () => {
  it('maps exec pins to arrow shape and exec edge kind', () => {
    expect(resolvePinVisualSpec({ type: 'exec' })).toMatchObject({
      shape: 'exec',
      colorKey: 'exec',
      edgeKind: 'exec',
      dashedStroke: false,
    });
  });

  it('maps scalar data types to circle shape', () => {
    expect(
      resolvePinVisualSpec({ type: 'object', dataType: { kind: 'Float64' } }),
    ).toMatchObject({ shape: 'circle', colorKey: 'Float64', edgeKind: 'data' });

    expect(
      resolvePinVisualSpec({ type: 'object', dataType: { kind: 'Date' } }),
    ).toMatchObject({ shape: 'circle', colorKey: 'date' });

    expect(
      resolvePinVisualSpec({ type: 'object', dataType: { kind: 'Categorical' } }),
    ).toMatchObject({ shape: 'circle', colorKey: 'categorical' });
  });

  it('maps container types to shape and recurses color to inner scalar', () => {
    expect(
      resolvePinVisualSpec({
        type: 'object',
        dataType: { kind: 'DataSeries', inner: { kind: 'Float64' } },
      }),
    ).toMatchObject({
      shape: 'diamond',
      colorKey: 'Float64',
      container: 'dataseries',
    });

    expect(
      resolvePinVisualSpec({
        type: 'object',
        dataType: { kind: 'Array', inner: { kind: 'String' } },
      }),
    ).toMatchObject({
      shape: 'roundedRect',
      colorKey: 'string',
      container: 'array',
    });
  });

  it('maps DataFrame and Struct to dedicated shapes', () => {
    expect(
      resolvePinVisualSpec({ type: 'object', dataType: { kind: 'DataFrame' } }),
    ).toMatchObject({ shape: 'gridRect', colorKey: 'dataframe' });

    expect(
      resolvePinVisualSpec({
        type: 'object',
        dataType: { kind: 'Struct', inner: 'OLSModel' },
      }),
    ).toMatchObject({ shape: 'hexagon', colorKey: 'struct' });
  });

  it('marks OneOf pins with dashed stroke', () => {
    expect(
      resolvePinVisualSpec({
        type: 'object',
        dataType: {
          kind: 'OneOf',
          inner: [{ kind: 'Float64' }, { kind: 'String' }],
        },
      }),
    ).toMatchObject({ shape: 'circle', colorKey: 'oneof', dashedStroke: true });
  });

});
