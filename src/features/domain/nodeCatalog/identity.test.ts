import { describe, expect, it } from 'vitest';
import {
  BUILTIN_NODE_TYPE_IDS,
  isCallFunctionNodeType,
  isDatabaseResourceNodeType,
  isVariableNodeType,
} from './identity';

describe('stable node identity', () => {
  it('classifies stable IDs and rejects every legacy display identity', () => {
    expect(isCallFunctionNodeType(BUILTIN_NODE_TYPE_IDS.callFunction)).toBe(true);
    expect(isVariableNodeType(BUILTIN_NODE_TYPE_IDS.getVariable)).toBe(true);
    expect(isVariableNodeType(BUILTIN_NODE_TYPE_IDS.setVariable)).toBe(true);
    expect(isDatabaseResourceNodeType(BUILTIN_NODE_TYPE_IDS.getDataframe)).toBe(true);

    const legacyIdentities = [
      'Functions:Call Function',
      'Variables:Get Variable',
      'Variables:Set Variable',
      'Data:Get DataFrame',
    ];
    for (const identity of legacyIdentities) {
      expect(isCallFunctionNodeType(identity)).toBe(false);
      expect(isVariableNodeType(identity)).toBe(false);
      expect(isDatabaseResourceNodeType(identity)).toBe(false);
    }
  });
});
