import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { GraphService } from './graphService';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('GraphService errors', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('does not catch-log-rethrow graph-removal failures', async () => {
    const transportError = new Error('transport unavailable');
    vi.mocked(invoke).mockRejectedValue(transportError);

    await expect(GraphService.removeGraph(
      'project-instance-current',
      'events/Main.yssbi-event',
      4,
      '00000000-0000-0000-0000-000000000123',
    )).rejects.toMatchObject({
      kind: 'transport',
      command: 'remove_graph',
      cause: transportError,
    });
  });
});
