import { invoke } from '@tauri-apps/api/core';
import { describe, expect, it, vi } from 'vitest';
import type { LocalizedCatalogDto } from './catalogService';
import { CatalogService } from './catalogService';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('CatalogService', () => {
  it('requests the localized catalog with the project identity and locale', async () => {
    const catalog: LocalizedCatalogDto = {
      projectInstanceId: 'project-instance-1',
      registryFingerprint: 'registry-fingerprint-1',
      resourcePublicationRevision: 7,
      locale: 'zh-CN',
      categories: [
        {
          categoryId: 'numeric',
          title: '数值',
          searchText: 'numeric 数值',
        },
      ],
      items: [
        {
          nodeTypeId: 'yssbi.numeric.add.int64',
          title: '整数相加',
          description: '将两个整数相加。',
          documentation: null,
          categoryId: 'numeric',
          iconId: 'numeric',
          styleId: 'default',
          aliases: ['加法'],
          technicalTerms: ['Int64'],
          pinyin: 'zheng shu xiang jia',
          ports: [
            { key: 'a', label: 'A', direction: 'input', kind: 'data' },
          ],
          parameters: [
            { key: 'value', title: '值', description: null },
          ],
          creation: {
            kind: 'static',
            nodeTypeId: 'yssbi.numeric.add.int64',
          },
          searchText: 'yssbi numeric add int64 整数相加 加法 int64',
        },
      ],
    };
    vi.mocked(invoke).mockResolvedValue(catalog);

    await expect(
      CatalogService.getLocalizedCatalog('project-instance-1', 'zh-CN'),
    ).resolves.toBe(catalog);

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith('get_localized_node_catalog', {
      projectInstanceId: 'project-instance-1',
      locale: 'zh-CN',
    });
  });

  it('accepts an exact parameterized-static descriptor without reconstructing it', async () => {
    const creation = {
      kind: 'parameterizedStatic' as const,
      nodeTypeId: 'yssbi.dataframe.project',
      requiredParameters: ['columns'],
    };
    vi.mocked(invoke).mockResolvedValue({
      projectInstanceId: 'project-instance-1',
      registryFingerprint: 'registry-fingerprint-1',
      resourcePublicationRevision: 8,
      locale: 'en-US',
      categories: [{ categoryId: 'dataframe', title: 'DataFrame', searchText: 'dataframe' }],
      items: [{
        nodeTypeId: 'yssbi.dataframe.project',
        title: 'Project DataFrame',
        description: 'Selects columns.',
        documentation: 'Selects direct columns.',
        categoryId: 'dataframe',
        iconId: 'builtin.dataframe',
        styleId: 'builtin.dataframe',
        aliases: ['select columns'],
        technicalTerms: ['select columns'],
        ports: [],
        parameters: [{ key: 'columns', title: 'Columns', description: null }],
        creation,
        searchText: 'project dataframe select columns',
      }],
    });

    const catalog = await CatalogService.getLocalizedCatalog('project-instance-1', 'en-US');
    expect(catalog.items[0].creation).toEqual(creation);
  });

  it('accepts the exact resource-bound item metadata and descriptor', async () => {
    const catalog: LocalizedCatalogDto = {
      projectInstanceId: 'project-instance-1',
      registryFingerprint: 'registry-fingerprint-1',
      resourcePublicationRevision: 8,
      locale: 'en-US',
      categories: [{ categoryId: 'functions', title: 'Functions', searchText: 'functions' }],
      items: [{
        nodeTypeId: 'yssbi.function.call',
        title: 'Call Helper',
        description: 'Calls Helper.',
        documentation: '# Helper',
        categoryId: 'functions',
        iconId: 'function',
        styleId: 'call',
        aliases: ['Helper'],
        technicalTerms: [],
        ports: [{ key: 'exec', label: 'Exec', direction: 'input', kind: 'execution' }],
        parameters: [{ key: 'target', title: 'Target', description: null }],
        resourcePath: 'functions/Helper.yssbi-function',
        resourceRevision: 3,
        creation: {
          kind: 'resourceBound',
          nodeTypeId: 'yssbi.function.call',
          resourcePath: 'functions/Helper.yssbi-function',
          resourceRevision: 3,
          createArgs: { kind: 'function' },
        },
        searchText: 'call helper',
      }],
    };
    vi.mocked(invoke).mockResolvedValue(catalog);

    await expect(CatalogService.getLocalizedCatalog('project-instance-1', 'en-US'))
      .resolves.toBe(catalog);
  });

  it('rejects resource metadata that does not exactly match its descriptor', async () => {
    vi.mocked(invoke).mockResolvedValue({
      projectInstanceId: 'project-instance-1',
      registryFingerprint: 'registry-fingerprint-1',
      resourcePublicationRevision: 8,
      locale: 'en-US',
      categories: [],
      items: [{
        nodeTypeId: 'function.call', title: 'Call A', description: null, documentation: null,
        categoryId: 'functions', iconId: 'function', styleId: 'call', aliases: [],
        technicalTerms: [], ports: [], parameters: [],
        resourcePath: 'functions/A', resourceRevision: 2,
        creation: {
          kind: 'resourceBound', nodeTypeId: 'function.call', resourcePath: 'functions/B',
          resourceRevision: 2, createArgs: { kind: 'function' },
        },
        searchText: 'call a',
      }],
    });

    await expect(CatalogService.getLocalizedCatalog('project-instance-1', 'en-US'))
      .rejects.toThrow('Invalid localized node catalog response');
  });

  it.each([
    ['missing static field', { kind: 'static' }],
    ['extra static field', { kind: 'static', nodeTypeId: 'math.add', extra: true }],
    ['missing parameterized field', {
      kind: 'parameterizedStatic', nodeTypeId: 'yssbi.dataframe.project',
    }],
    ['extra parameterized field', {
      kind: 'parameterizedStatic', nodeTypeId: 'yssbi.dataframe.project',
      requiredParameters: ['columns'], parameters: {},
    }],
    ['wrong parameterized key list', {
      kind: 'parameterizedStatic', nodeTypeId: 'yssbi.dataframe.project',
      requiredParameters: 'columns',
    }],
    ['missing resource field', {
      kind: 'resourceBound', nodeTypeId: 'function.call', resourcePath: 'functions/A',
      resourceRevision: 1,
    }],
    ['extra resource field', {
      kind: 'resourceBound', nodeTypeId: 'function.call', resourcePath: 'functions/A',
      resourceRevision: 1, createArgs: { kind: 'function' }, extra: true,
    }],
    ['extra create args field', {
      kind: 'resourceBound', nodeTypeId: 'function.call', resourcePath: 'functions/A',
      resourceRevision: 1, createArgs: { kind: 'function', extra: true },
    }],
  ])('rejects a descriptor with %s', async (_label, creation) => {
    vi.mocked(invoke).mockResolvedValue({
      projectInstanceId: 'project-instance-1',
      registryFingerprint: 'registry-fingerprint-1',
      resourcePublicationRevision: 8,
      locale: 'en-US',
      categories: [],
      items: [{
        nodeTypeId: 'function.call',
        title: 'Call A',
        description: null,
        documentation: null,
        categoryId: 'functions',
        iconId: 'function',
        styleId: 'call',
        aliases: [],
        technicalTerms: [],
        ports: [],
        parameters: [],
        creation,
        searchText: 'call a',
      }],
    });

    await expect(CatalogService.getLocalizedCatalog('project-instance-1', 'en-US'))
      .rejects.toThrow('Invalid localized node catalog response');
  });
});
