import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SourceService } from './resultSourceService';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('SourceService opaque ID contract', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockResolvedValue(null);
  });

  it('sends decimal string source IDs to every result-source command', async () => {
    const sourceId = '9007199254740993';

    await SourceService.getDescriptor(sourceId);
    await SourceService.getValue(sourceId);
    await SourceService.getPage(sourceId, 10, 20);
    await SourceService.releaseResultSource(sourceId);

    expect(vi.mocked(invoke).mock.calls).toEqual([
      ['get_result_source_descriptor', { sourceId }],
      ['get_result_source_value', { sourceId }],
      ['get_result_source_page', { sourceId, offset: 10, limit: 20 }],
      ['release_result_source', { sourceId }],
    ]);
  });
});
