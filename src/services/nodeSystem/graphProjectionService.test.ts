import { invoke } from '@tauri-apps/api/core';
import { describe, expect, it, vi } from 'vitest';
import type { EditorGraphProjectionDto } from '@/shared/types/dto/editorProjection';
import { GraphProjectionService } from './graphProjectionService';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('GraphProjectionService', () => {
  it('loads and hydrates graph projections with graph path and locale', async () => {
    const projection = {} as EditorGraphProjectionDto;
    vi.mocked(invoke).mockResolvedValue(projection);

    await expect(
      GraphProjectionService.loadGraph('functions/main', 'zh-CN', 7, 'project-instance-1'),
    ).resolves.toBe(projection);
    expect(invoke).toHaveBeenLastCalledWith('load_project_graph', {
      graphPath: 'functions/main',
      locale: 'zh-CN',
      lifecycleToken: 7,
      projectInstanceId: 'project-instance-1',
    });

    await expect(GraphProjectionService.hydrateGraph('functions/main', 'en-US')).resolves.toBe(projection);
    expect(invoke).toHaveBeenLastCalledWith('hydrate_editor_graph', {
      graphPath: 'functions/main',
      locale: 'en-US',
    });
  });
});
