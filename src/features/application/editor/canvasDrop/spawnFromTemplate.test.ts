import { describe, expect, it, vi } from 'vitest';
import type { EditorFunctions, EditorVariables } from '@/features/core/editor';
import { CALL_FUNCTION_NODE_TYPE } from '@/features/domain/nodeDefinition';
import {
  dataFrameNodeSpawnTemplate,
  functionCallNodeSpawnTemplate,
  variableNodeSpawnTemplate,
} from '@/features/core/dnd/nodeSpawnTemplate';
import { spawnNodeFromTemplate } from './spawnFromTemplate';

describe('spawnNodeFromTemplate', () => {
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
    'fn-1': {
      id: 'fn-1',
      name: 'Add',
      functionInputs: [],
      functionOutputs: [],
    },
  };

  it('spawns builtin node with nodeType only', async () => {
    const createNode = vi.fn().mockResolvedValue({ nodeId: 'n1', pinIds: [] });
    await spawnNodeFromTemplate(
      { nodeType: 'Math:Add', title: 'Add' },
      { x: 10, y: 20 },
      { x: 100, y: 200 },
      { altKey: false, ctrlKey: false },
      { variables, functions, createNode, onVariableMenu: vi.fn() },
    );
    expect(createNode).toHaveBeenCalledWith('Math:Add', { x: 10, y: 20 });
  });

  it('spawns dataframe node with dataframeId params', async () => {
    const createNode = vi.fn().mockResolvedValue({ nodeId: 'n1', pinIds: [] });
    await spawnNodeFromTemplate(
      dataFrameNodeSpawnTemplate('df-1', 'sales'),
      { x: 1, y: 2 },
      { x: 3, y: 4 },
      { altKey: false, ctrlKey: false },
      { variables, functions, createNode, onVariableMenu: vi.fn() },
    );
    expect(createNode).toHaveBeenCalledWith('Data:Get DataFrame', { x: 1, y: 2 }, {
      dataframeId: 'df-1',
      variableName: 'sales',
    });
  });

  it('spawns get-variable when ctrl is held', async () => {
    const createNode = vi.fn().mockResolvedValue({ nodeId: 'n1', pinIds: [] });
    await spawnNodeFromTemplate(
      variableNodeSpawnTemplate('var-1', 'Counter'),
      { x: 5, y: 6 },
      { x: 50, y: 60 },
      { altKey: false, ctrlKey: true },
      { variables, functions, createNode, onVariableMenu: vi.fn() },
    );
    expect(createNode).toHaveBeenCalledWith(
      'Variables:Get Variable',
      { x: 5, y: 6 },
      { variableId: 'var-1' },
    );
  });

  it('spawns function call when subGraph exists', async () => {
    const createNode = vi.fn().mockResolvedValue({ nodeId: 'n1', pinIds: [] });
    await spawnNodeFromTemplate(
      functionCallNodeSpawnTemplate('fn-1', 'Add'),
      { x: 0, y: 0 },
      { x: 0, y: 0 },
      { altKey: false, ctrlKey: false },
      { variables, functions, createNode, onVariableMenu: vi.fn() },
    );
    expect(createNode).toHaveBeenCalledWith(
      CALL_FUNCTION_NODE_TYPE,
      { x: 0, y: 0 },
      { subGraphId: 'fn-1' },
    );
  });
});
