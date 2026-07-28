// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
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

  it('does not report requested node IDs when delete sequencing fails', async () => {
    executeCommand.mockResolvedValueOnce(false);

    await expect(management.deleteNodes(['node-1', 'node-2'])).resolves.toEqual([]);
  });

  it('returns false when a single delete is not applied', async () => {
    executeCommand.mockResolvedValueOnce(false);

    await expect(management.deleteNode('node-1')).resolves.toBe(false);
  });
});
