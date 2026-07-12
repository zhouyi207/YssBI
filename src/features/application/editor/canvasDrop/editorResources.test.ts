import { describe, expect, it } from 'vitest';
import type { EditorFunctions, EditorVariables } from '@/features/core/editor';
import { isFunctionAvailable, isVariableAvailable } from './editorResources';

describe('canvasDrop editorResources', () => {
  it('isVariableAvailable checks scoped catalog and store fallback', () => {
    const variables: EditorVariables = {
      'var-1': {
        id: 'var-1',
        name: 'Counter',
        dataType: { kind: 'Int64' },
        dataValue: { kind: 'Int64', value: 0 },
        description: '',
        scope: { type: 'global' },
        tags: [],
      },
    };

    expect(isVariableAvailable('var-1', variables)).toBe(true);
    expect(isVariableAvailable('missing', variables)).toBe(false);
  });

  it('isFunctionAvailable checks function catalog keys', () => {
    const functions: EditorFunctions = {
      'fn-1': {
        id: 'fn-1',
        name: 'Add',
        functionInputs: [],
        functionOutputs: [],
      },
    };

    expect(isFunctionAvailable('fn-1', functions)).toBe(true);
    expect(isFunctionAvailable('fn-2', functions)).toBe(false);
  });
});
