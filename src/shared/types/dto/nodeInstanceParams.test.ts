import { describe, expect, it } from 'vitest';
import {
  NODE_INSTANCE_PARAMS_NONE,
  nodeSpawnFieldsToInstanceParams,
  spawnParamsToInstanceParams,
} from './nodeInstanceParams';

describe('nodeInstanceParams', () => {
  it('spawnParamsToInstanceParams maps flat fields to tagged union', () => {
    expect(spawnParamsToInstanceParams({ variableId: 'v1', variableName: 'x' })).toEqual({
      paramsKind: 'variable',
      variableId: 'v1',
      variableName: 'x',
    });
    expect(spawnParamsToInstanceParams({ subGraphPath: 'fn-1' })).toEqual({
      paramsKind: 'subGraph',
      subGraphPath: 'fn-1',
    });
    expect(spawnParamsToInstanceParams(undefined)).toBeNull();
    expect(spawnParamsToInstanceParams({})).toEqual(NODE_INSTANCE_PARAMS_NONE);
  });

  it('nodeSpawnFieldsToInstanceParams respects paramsKind discriminator', () => {
    expect(
      nodeSpawnFieldsToInstanceParams({
        paramsKind: 'dataFrame',
        dataframeId: 'df-1',
      }),
    ).toEqual({ paramsKind: 'dataFrame', dataframeId: 'df-1' });
  });
});
