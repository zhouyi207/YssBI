import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { logger } from '@/utils/appLogger';

import { GraphService } from './graphService';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('GraphService errors', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(logger.graph, 'error').mockImplementation(() => undefined);
  });

  it('logs a structured Tauri function-creation error message', async () => {
    const error = {
      code: 'resource_revision_conflict',
      message: 'resource revision conflict: function recreation changed',
    };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(GraphService.createFunction(
      'project-instance-current',
      'New Function',
      '00000000-0000-0000-0000-000000000122',
    )).rejects.toBe(error);

    expect(logger.graph.error).toHaveBeenCalledWith(
      `Error creating function: ${error.message}`,
      'GraphService',
    );
  });

  it('logs a structured Tauri graph-removal error message', async () => {
    const error = {
      code: 'resource_revision_conflict',
      message: "resource revision conflict: revision for 'events/Main.yssbi-event' changed",
    };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(GraphService.removeGraph(
      'project-instance-current',
      'events/Main.yssbi-event',
      4,
      '00000000-0000-0000-0000-000000000123',
    )).rejects.toBe(error);

    expect(logger.graph.error).toHaveBeenCalledWith(
      `Error removing graph: ${error.message}`,
      'GraphService',
    );
  });
});
