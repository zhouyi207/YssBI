import { describe, expect, it, vi } from 'vitest';
import type { EditorFunctions, EditorVariables } from '@/features/core/editor';
import {
  dataFrameNodeSpawnTemplate,
  functionCallNodeSpawnTemplate,
  variableNodeSpawnTemplate,
} from '@/features/core/dnd/nodeSpawnTemplate';
import { spawnNodeFromTemplate } from './spawnFromTemplate';

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
const functions: EditorFunctions = {
  'fn-1': { id: 'fn-1', name: 'Add', functionInputs: [], functionOutputs: [] },
};

describe('spawnNodeFromTemplate unavailable capability', () => {
  it.each([
    { nodeType: 'Math:Add', title: 'Add' },
    dataFrameNodeSpawnTemplate('df-1', 'sales'),
    variableNodeSpawnTemplate('var-1', 'Counter'),
    functionCallNodeSpawnTemplate('fn-1', 'Add'),
  ])('does not call createNode for $nodeType', async (template) => {
    const createNode = vi.fn();

    await expect(spawnNodeFromTemplate(
      template,
      { x: 10, y: 20 },
      { x: 100, y: 200 },
      { altKey: false, ctrlKey: true },
      { variables, functions, createNode, onVariableMenu: vi.fn() },
    )).resolves.toBe(false);

    expect(createNode).not.toHaveBeenCalled();
  });
});
