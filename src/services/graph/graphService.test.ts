import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IpcError } from '@/services/ipc';
import { GraphService } from './graphService';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('GraphService errors', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('surfaces a structured function-creation rejection as IpcError', async () => {
    const error = {
      code: 'resource_revision_conflict',
      details: { resourcePath: 'functions/New Function.yssbi-function' },
      incidentId: 'incident-function-creation',
    };
    vi.mocked(invoke).mockRejectedValue(error);

    const caught = await GraphService.createFunction(
      'project-instance-current',
      'New Function',
      '00000000-0000-0000-0000-000000000122',
    ).catch((caughtError: unknown) => caughtError);

    expect(caught).toBeInstanceOf(IpcError);
    expect(caught).toMatchObject({
      kind: 'backend',
      command: 'create_function',
      code: error.code,
      details: error.details,
      incidentId: error.incidentId,
      cause: error,
    });
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
