import { beforeEach, describe, expect, it } from 'vitest';
import { useGraphMetaStore } from './graphMetaStore';
import { syncFunctionSignatureMeta } from './graphDocumentMeta';

describe('graphDocumentMeta', () => {
  beforeEach(() => {
    useGraphMetaStore.setState({ graphs: {}, graphOrder: [], graphFolders: [] });
  });

  it('ignores event graphs when syncing function signature metadata', () => {
    syncFunctionSignatureMeta({
      id: 'event-1',
      name: 'Event',
      type: 'event',
      functionInputs: [{ id: 'input-1', name: 'Value', type: 'int' }],
      functionOutputs: [{ id: 'output-1', name: 'Result', type: 'float' }],
    });

    expect(useGraphMetaStore.getState().graphs['event-1']).toBeUndefined();
  });
});
