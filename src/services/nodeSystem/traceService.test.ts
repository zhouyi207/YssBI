import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TraceRecordDto } from '@/shared/types/dto/trace';
import { TraceService } from './traceService';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const traceRecord: TraceRecordDto = {
  sequence: '9007199254740993',
  kind: 'run',
  status: 'succeeded',
  correlation: {
    projectSessionId: 'project-session-1',
    graphPath: 'events/Main.yssbi-event',
    graphRevision: '7',
    registryFingerprint: 'registry-fingerprint-1',
    resourceVersions: { dataset: 'version-1' },
    compileId: '9007199254740994',
    runId: '9007199254740995',
    nodeId: null,
    nodeTypeId: null,
    parentCall: null,
  },
  fields: {
    backendId: { type: 'text', value: 'datafusion' },
    subplanIndex: { type: 'integer', value: 2 },
    secret: { type: 'redacted' },
  },
};

describe('TraceService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockResolvedValue([traceRecord]);
  });

  it('lists graph traces with the exact project and graph arguments', async () => {
    await expect(
      TraceService.listGraphTraces('project-instance-1', 'events/Main.yssbi-event'),
    ).resolves.toEqual([traceRecord]);

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith('list_graph_traces', {
      projectInstanceId: 'project-instance-1',
      graphPath: 'events/Main.yssbi-event',
    });
  });

  it('gets a run trace with an opaque decimal-string run ID', async () => {
    const runId = '9007199254740995';

    await expect(
      TraceService.getRunTrace('project-instance-1', runId),
    ).resolves.toEqual([traceRecord]);

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith('get_run_trace', {
      projectInstanceId: 'project-instance-1',
      runId,
    });
  });
});
