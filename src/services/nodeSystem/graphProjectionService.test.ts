import { readFileSync } from 'node:fs';
import { invoke } from '@tauri-apps/api/core';
import { describe, expect, it, vi } from 'vitest';
import editorProjection from '@/tests/fixtures/node-system-contracts/editor-projection.json';
import { GraphProjectionService } from './graphProjectionService';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('GraphProjectionService', () => {
  it('keeps the service boundary independent from features', () => {
    const source = readFileSync(new URL('./graphProjectionService.ts', import.meta.url), 'utf8');
    expect(source).not.toMatch(/from\s+['"]@\/features(?:\/|['"])/);
  });

  it('loads the authoritative projection with unchanged command arguments', async () => {
    vi.mocked(invoke).mockResolvedValue(editorProjection as unknown);

    await expect(
      GraphProjectionService.loadGraph('functions/main', 'zh-CN', 7, 'project-instance-1'),
    ).resolves.toEqual(editorProjection);
    expect(invoke).toHaveBeenLastCalledWith('load_project_graph', {
      graphPath: 'functions/main',
      locale: 'zh-CN',
      lifecycleToken: 7,
      projectInstanceId: 'project-instance-1',
    });
  });

  it('hydrates the authoritative projection with unchanged command arguments', async () => {
    vi.mocked(invoke).mockResolvedValue(editorProjection as unknown);

    await expect(
      GraphProjectionService.hydrateGraph('project-instance-1', 'functions/main', 'en-US'),
    ).resolves.toEqual(editorProjection);
    expect(invoke).toHaveBeenLastCalledWith('hydrate_editor_graph', {
      projectInstanceId: 'project-instance-1',
      graphPath: 'functions/main',
      locale: 'en-US',
    });
  });


  const requests = [
    ['loadGraph', () => GraphProjectionService.loadGraph(
      'events/contract.yssbi-event', 'en-US', 7, 'project-instance-1',
    )],
    ['hydrateGraph', () => GraphProjectionService.hydrateGraph(
      'project-instance-1', 'events/contract.yssbi-event', 'en-US',
    )],
  ] as const;

  it.each(requests)('rejects a malformed root from %s with the public error', async (_name, request) => {
    vi.mocked(invoke).mockResolvedValue({ ...editorProjection, compatibility: true } as unknown);
    await expect(request()).rejects.toThrow('Invalid editor graph projection response');
  });

  it.each(requests)(
    'rejects malformed nested parameter configuration from %s with the public error',
    async (_name, request) => {
      const malformed = structuredClone(editorProjection) as unknown as {
        nodes: Array<{ parameterEditors: Array<{ configuration: unknown }> }>;
      };
      malformed.nodes[0].parameterEditors[0].configuration = {
        kind: 'projectColumns',
        available: true,
        unavailableReason: null,
        options: [],
        value: [],
        compatibility: true,
      };
      vi.mocked(invoke).mockResolvedValue(malformed as unknown);

      await expect(request()).rejects.toThrow('Invalid editor graph projection response');
    },
  );
});
