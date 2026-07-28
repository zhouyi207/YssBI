// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useEditorKeyboard } from './useEditorKeyboard';
import { useEditorOperations } from './useEditorOperations';

const executeCommand = vi.hoisted(() => vi.fn());
const updateSelected = vi.hoisted(() => vi.fn());
const setClipboard = vi.hoisted(() => vi.fn());
let selectedNodeIds: string[] = [];

vi.mock('@/features/core/history', () => ({
  executeCommand,
  useHistoryStore: (selector: (state: { canUndo: boolean; canRedo: boolean; pending: boolean }) => unknown) =>
    selector({ canUndo: false, canRedo: false, pending: false }),
}));

vi.mock('@/features/core/editor', () => ({
  useClipboardStore: (selector: (state: { setClipboard: typeof setClipboard }) => unknown) =>
    selector({ setClipboard }),
}));
vi.mock('@/features/core/layout', () => ({
  updateEditorGroupSelectedNodeIds: updateSelected,
}));
vi.mock('@/features/core/editor/hooks/useActiveEditorGroup', () => ({
  useActiveEditorGroup: () => ({ activeTabId: graphPath, selectedNodeIds }),
}));
vi.mock('@/features/application/editorMutation/historyCoordinator', () => ({
  undoEditorHistory: vi.fn(),
  redoEditorHistory: vi.fn(),
  setHistoryStatus: vi.fn(),
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const graphPath = 'events/delete-capabilities';
const nodeId = 'projected-node';

function installNode(canCopy: boolean | undefined, canDelete: boolean, managed = false) {
  const fixture = makeEditorProjectionFixture({ graphPath, nodeId });
  const capabilities = {
    ...fixture.projection.nodes[0].capabilities,
    managed,
    canCopy: canCopy ?? true,
    canDelete,
  };
  if (canCopy === undefined) {
    delete (capabilities as { canCopy?: boolean }).canCopy;
  }
  fixture.projection.nodes[0].capabilities = capabilities;
  useGraphDataStore.getState().replaceProjection(graphPath, fixture.projection, 1);
  const storedNode = useGraphDataStore.getState().getGraphNode(graphPath, nodeId);
  if (storedNode) storedNode.isInternal = false;
}

describe('useEditorOperations projected deletion capabilities', () => {
  let host: HTMLDivElement;
  let root: Root;
  let operations: ReturnType<typeof useEditorOperations>;

  function Harness(): null {
    operations = useEditorOperations();
    useEditorKeyboard({
      deleteSelected: operations.deleteSelected,
      undo: operations.undo,
      redo: operations.redo,
      copy: operations.copy,
      cut: operations.cut,
      paste: operations.paste,
      duplicateSelected: operations.duplicateSelected,
      saveGraph: () => undefined,
      saveGraphAs: () => undefined,
      importGraph: () => undefined,
      addEvent: () => undefined,
      closeTab: () => undefined,
      setActiveTabId: () => undefined,
      splitEditorRight: () => undefined,
    });
    return null;
  }

  beforeEach(() => {
    vi.clearAllMocks();
    executeCommand.mockResolvedValue(true);
    selectedNodeIds = [nodeId];
    useGraphDataStore.setState({ graphEntities: {} });
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  async function renderOperations() {
    await act(async () => root.render(<Harness />));
  }

  it.each([
    { label: 'inconsistent canCopy=true', canCopy: true },
    { label: 'missing canCopy', canCopy: undefined },
  ])('rejects direct copy and Ctrl+C for a managed node with $label', async ({ canCopy }) => {
    installNode(canCopy, false, true);
    await renderOperations();

    operations.copy();
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', {
        key: 'c',
        ctrlKey: true,
        bubbles: true,
        cancelable: true,
      }));
    });

    expect(setClipboard).not.toHaveBeenCalled();
  });

  it('never submits deletion for a Rust-managed node', async () => {
    installNode(false, false, true);
    await renderOperations();

    await operations.deleteSelected();
    await operations.deleteNodesById([nodeId]);
    await operations.cut();
    await operations.cutNodes([nodeId]);

    expect(executeCommand).not.toHaveBeenCalled();
  });

  it.each([
    { canCopy: false, canDelete: true },
    { canCopy: true, canDelete: false },
  ])('does not cut unless copy and delete are both allowed: %o', async ({ canCopy, canDelete }) => {
    installNode(canCopy, canDelete);
    await renderOperations();

    await operations.cutNodes([nodeId]);

    expect(executeCommand).not.toHaveBeenCalled();
  });

  it('preserves deletion and cut for an ordinary projected node', async () => {
    installNode(true, true);
    await renderOperations();

    await operations.deleteSelected();
    await operations.cutNodes([nodeId]);

    expect(executeCommand).toHaveBeenNthCalledWith(
      1,
      graphPath,
      'DeleteNodes',
      { nodeIds: [nodeId] },
    );
    expect(executeCommand).toHaveBeenNthCalledWith(
      2,
      graphPath,
      'DeleteNodes',
      { nodeIds: [nodeId] },
    );
  });
});
