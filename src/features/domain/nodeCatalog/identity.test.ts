import { describe, expect, it } from 'vitest';
import {
  BUILTIN_NODE_TYPE_IDS,
  isCallFunctionNodeType,
  isDatabaseResourceNodeType,
  isVariableNodeType,
} from './identity';

describe('stable node identity', () => {
  it('uses the Rust-defined built-in node type IDs', () => {
    expect(BUILTIN_NODE_TYPE_IDS).toEqual({
      callFunction: 'yssbi.project.function.call',
      getVariable: 'yssbi.project.variable.get',
      setVariable: 'yssbi.project.variable.set',
      getDataframe: 'yssbi.dataframe.source.get',
    });
  });

  it('classifies stable IDs and rejects every legacy display identity', () => {
    expect(isCallFunctionNodeType(BUILTIN_NODE_TYPE_IDS.callFunction)).toBe(true);
    expect(isVariableNodeType(BUILTIN_NODE_TYPE_IDS.getVariable)).toBe(true);
    expect(isVariableNodeType(BUILTIN_NODE_TYPE_IDS.setVariable)).toBe(true);
    expect(isDatabaseResourceNodeType(BUILTIN_NODE_TYPE_IDS.getDataframe)).toBe(true);

    expect(isCallFunctionNodeType('Functions:Call Function')).toBe(false);
    expect(isVariableNodeType('Variables:Get Variable')).toBe(false);
    expect(isVariableNodeType('Variables:Set Variable')).toBe(false);
    expect(isDatabaseResourceNodeType('Data:Get DataFrame')).toBe(false);

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
