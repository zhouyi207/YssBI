import { describe, expect, it } from 'vitest';
import {
  toBatchCreateNodeIpcItems,
  spawnParamsToInstanceParams,
  type BatchCreateNodeRequest,
} from './batchCreateNode';

describe('batchCreateNode DTO', () => {
  it('maps spawn params to tagged NodeInstanceParams', () => {
    expect(spawnParamsToInstanceParams({ variableId: 'v1' })).toEqual({
      paramsKind: 'variable',
      variableId: 'v1',
    });
    expect(spawnParamsToInstanceParams({ subGraphId: 'fn-1' })).toEqual({
      paramsKind: 'subGraph',
      subGraphId: 'fn-1',
    });
    expect(spawnParamsToInstanceParams(undefined)).toBeNull();
  });

  it('serializes batch requests for IPC', () => {
    const requests: BatchCreateNodeRequest[] = [
      { nodeType: 'Math:Add', x: 10, y: 20 },
      { nodeType: 'Variables:Get Variable', params: { variableId: 'v1' } },
    ];
    expect(toBatchCreateNodeIpcItems(requests)).toEqual([
      { nodeType: 'Math:Add', x: 10, y: 20, params: null },
      {
        nodeType: 'Variables:Get Variable',
        x: null,
        y: null,
        params: { paramsKind: 'variable', variableId: 'v1' },
      },
    ]);
  });
});
