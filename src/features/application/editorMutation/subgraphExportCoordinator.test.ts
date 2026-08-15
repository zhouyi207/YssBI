import { beforeEach, describe, expect, it, vi } from 'vitest';
import { clearProjectLifecycle, startProjectLifecycle } from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { GraphSubgraphService } from '@/services/nodeSystem/graphSubgraphService';
import type { ClipboardSubgraphDto } from '@/shared/types/dto/clipboardSubgraph';
import { exportEditorSubgraph } from './subgraphExportCoordinator';

const snapshot: ClipboardSubgraphDto = {
  schemaVersion: 1,
  nodes: [],
  portBindings: [],
  inputStates: [],
  connections: [],
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}

describe('exportEditorSubgraph', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    clearProjectLifecycle();
    startProjectLifecycle('project-a');
  });

  it('captures the project identity for the read-only export', async () => {
    vi.spyOn(GraphSubgraphService, 'exportSubgraph').mockResolvedValue(snapshot);

    await expect(exportEditorSubgraph({
      graphPath: 'events/main.yssbi-event',
      nodeIds: ['node-a'],
    })).resolves.toBe(snapshot);

    expect(GraphSubgraphService.exportSubgraph).toHaveBeenCalledWith(
      'project-a',
      'events/main.yssbi-event',
      ['node-a'],
    );
  });

  it('rejects an old response after project replacement before clipboard writing', async () => {
    const pending = deferred<ClipboardSubgraphDto>();
    vi.spyOn(GraphSubgraphService, 'exportSubgraph').mockReturnValue(pending.promise);
    const writeClipboard = vi.fn();
    const operation = exportEditorSubgraph({
      graphPath: 'events/main.yssbi-event',
      nodeIds: ['node-a'],
    }).then(writeClipboard);

    startProjectLifecycle('project-b');
    pending.resolve(snapshot);

    await expect(operation).rejects.toMatchObject({ code: 'stale_project_lifecycle' });
    expect(writeClipboard).not.toHaveBeenCalled();
  });
});
