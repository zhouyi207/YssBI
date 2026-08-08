import { describe, expect, it, vi } from 'vitest';
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import {
  findResourceNodeSpawnTemplate,
  spawnNodeFromTemplate,
} from './spawnFromTemplate';

describe('spawnNodeFromTemplate', () => {
  it.each([
    {
      label: 'static',
      descriptor: { kind: 'static', nodeTypeId: 'math.add' },
    },
    {
      label: 'function',
      descriptor: {
        kind: 'resourceBound',
        nodeTypeId: 'yssbi.project.function.call',
        resourcePath: 'functions/Helper.yssbi-function',
        resourceRevision: 4,
        createArgs: { kind: 'function' },
      },
    },
    {
      label: 'variable',
      descriptor: {
        kind: 'resourceBound',
        nodeTypeId: 'yssbi.project.variable.get',
        resourcePath: 'variables/00000000-0000-0000-0000-000000000001',
        resourceRevision: 5,
        createArgs: { kind: 'variable' },
      },
    },
    {
      label: 'database',
      descriptor: {
        kind: 'resourceBound',
        nodeTypeId: 'yssbi.dataframe.source.get',
        resourcePath: 'databases/sales / . # 数据',
        resourceRevision: 6,
        createArgs: { kind: 'database' },
      },
    },
  ] satisfies Array<{ label: string; descriptor: NodeCreationDescriptor }>) (
    'forwards the exact $label descriptor without reconstruction',
    async ({ descriptor }) => {
      const createNode = vi.fn(async () => true);

      await expect(spawnNodeFromTemplate(
        { title: 'Spawn', descriptor },
        { x: 10, y: 20 },
        { createNode },
      )).resolves.toBe(true);

      expect(createNode).toHaveBeenCalledOnce();
      expect(createNode).toHaveBeenCalledWith(descriptor, { x: 10, y: 20 });
    },
  );

  it('looks up only the exact current opaque resource path and descriptor kind', () => {
    const descriptor: NodeCreationDescriptor = {
      kind: 'resourceBound',
      nodeTypeId: 'yssbi.project.function.call',
      resourcePath: 'functions/opaque / . # 数据',
      resourceRevision: 9,
      createArgs: { kind: 'function' },
    };
    const items = [{
      nodeTypeId: 'yssbi.project.function.call',
      title: 'Opaque',
      description: null,
      documentation: null,
      categoryId: 'functions',
      iconId: 'function',
      styleId: 'call',
      aliases: [],
      technicalTerms: [],
      backendSearchText: ['opaque'],
      resourceNames: ['Opaque'],
      ports: [],
      parameters: [],
      resourcePath: descriptor.resourcePath,
      resourceRevision: 9,
      creation: descriptor,
    }];

    expect(findResourceNodeSpawnTemplate(
      items,
      descriptor.resourcePath,
      'function',
      'yssbi.project.function.call',
    )).toEqual({ title: 'Opaque', descriptor });
    expect(findResourceNodeSpawnTemplate(
      items,
      descriptor.resourcePath,
      'function',
      'yssbi.project.variable.get',
    )).toBeNull();
    expect(findResourceNodeSpawnTemplate(items, 'functions/opaque', 'function')).toBeNull();
    expect(findResourceNodeSpawnTemplate(items, descriptor.resourcePath, 'database')).toBeNull();
  });

  it('returns false without retrying when descriptor creation is rejected', async () => {
    const descriptor: NodeCreationDescriptor = { kind: 'static', nodeTypeId: 'math.add' };
    const createNode = vi.fn(async () => false);

    await expect(spawnNodeFromTemplate(
      { descriptor },
      { x: 1, y: 2 },
      { createNode },
    )).resolves.toBe(false);

    expect(createNode).toHaveBeenCalledOnce();
  });
});
