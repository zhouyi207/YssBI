// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import { createNodeFromDescriptor } from '@/features/application/nodeCatalog/createNodeFromDescriptor';
import { useNodeManagement } from './useNodeManagement';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const executeCommand = vi.hoisted(() => vi.fn());
vi.mock('@/features/core/history', () => ({ executeCommand }));
vi.mock('@/features/core/editor/hooks/useActiveEditorGroup', () => ({
  useActiveEditorGroup: () => ({ activeTabId: 'events/main.yssbi-event' }),
}));
vi.mock('@/features/core/dataStore/graphNodeSelectors', () => ({
  canDeleteNode: () => true,
}));
vi.mock('@/features/application/nodeCatalog/createNodeFromDescriptor', () => ({
  createNodeFromDescriptor: vi.fn(),
}));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ i18n: { resolvedLanguage: 'zh-CN', language: 'zh-CN' } }),
}));

describe('useNodeManagement mutation outcomes', () => {
  let root: Root;
  let management!: ReturnType<typeof useNodeManagement>;

  beforeEach(() => {
    vi.clearAllMocks();
    root = createRoot(document.createElement('div'));
    function Harness() {
      management = useNodeManagement();
      return null;
    }
    act(() => root.render(<Harness />));
  });

  afterEach(() => act(() => root.unmount()));

  it('forwards an exact descriptor through the active graph mutation path', async () => {
    vi.mocked(createNodeFromDescriptor).mockResolvedValue({ status: 'conflict' });
    const descriptor: NodeCreationDescriptor = {
      kind: 'resourceBound',
      nodeTypeId: 'function.call',
      resourcePath: 'functions/Helper.yssbi-function',
      resourceRevision: 3,
      createArgs: { kind: 'function' },
    };

    await expect(management.createNode(descriptor, { x: 4, y: 9 })).resolves.toBe(false);

    expect(createNodeFromDescriptor).toHaveBeenCalledWith({
      graphPath: 'events/main.yssbi-event',
      locale: 'zh-CN',
      descriptor,
      position: { x: 4, y: 9 },
      connectFrom: null,
    });
  });

  it('submits a node collection once and does not report IDs when the intent fails', async () => {
    executeCommand.mockResolvedValueOnce(false);

    await expect(management.deleteNodes(['node-1', 'node-2'])).resolves.toEqual([]);

    expect(executeCommand).toHaveBeenCalledTimes(1);
    expect(executeCommand).toHaveBeenCalledWith(
      'events/main.yssbi-event',
      'DeleteNodes',
      { nodeIds: ['node-1', 'node-2'] },
    );
  });

  it('returns false when a single delete is not applied', async () => {
    executeCommand.mockResolvedValueOnce(false);

    await expect(management.deleteNode('node-1')).resolves.toBe(false);
  });
});
