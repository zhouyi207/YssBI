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
          aliases: ['加法'],
          technicalTerms: ['Int64'],
          pinyin: 'zheng shu xiang jia',
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
});
