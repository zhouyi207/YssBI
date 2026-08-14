// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useEditorKeyboard } from './useEditorKeyboard';
import { useEditorOperations } from './useEditorOperations';
import { pickEditorSessionNodeActions } from './editorSessionTypes';

const executeCommand = vi.hoisted(() => vi.fn());
const executeCommandOutcome = vi.hoisted(() => vi.fn());
const updateSelected = vi.hoisted(() => vi.fn());
const updateSelectedConnections = vi.hoisted(() => vi.fn());
const disconnectConnectionsById = vi.hoisted(() => vi.fn());
const setClipboard = vi.hoisted(() => vi.fn());
let selectedNodeIds: string[] = [];
let selectedConnectionIds: string[] = [];
let activeGraphPath = 'events/delete-capabilities';
let activeGroupId = 'group-a';

vi.mock('@/features/core/history', () => ({
  executeCommand,
  executeCommandOutcome,
  useHistoryStore: (selector: (state: { canUndo: boolean; canRedo: boolean; pending: boolean }) => unknown) =>
    selector({ canUndo: false, canRedo: false, pending: false }),
}));

vi.mock('@/features/core/editor', () => ({
  useClipboardStore: (selector: (state: { setClipboard: typeof setClipboard }) => unknown) =>
    selector({ setClipboard }),
}));
vi.mock('@/features/core/layout', () => ({
  updateEditorGroupSelectedNodeIds: updateSelected,
  updateEditorGroupSelectedConnectionIds: updateSelectedConnections,
}));
vi.mock('./edgeOperations', () => ({ disconnectConnectionsById }));
vi.mock('@/features/core/editor/hooks/useActiveEditorGroup', () => ({
  useActiveEditorGroup: () => ({
    activeTabId: activeGraphPath,
    groupId: activeGroupId,
    selectedNodeIds,
    selectedConnectionIds,
  }),
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

describe('editor session node action contract', () => {
  it('exposes only descriptor-backed creation and deletion actions', () => {
    const createNode = vi.fn();
    const deleteNode = vi.fn();
    const deleteNodes = vi.fn();
    const actions = pickEditorSessionNodeActions({
      createNode,
      createNodes: vi.fn(),
      deleteNode,
      deleteNodes,
    } as never);

    expect(actions).toEqual({ createNode, deleteNode, deleteNodes });
  });
});

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
    executeCommandOutcome.mockResolvedValue({ status: 'applied' });
    vi.clearAllMocks();
    executeCommand.mockResolvedValue(true);
    selectedNodeIds = [nodeId];
    selectedConnectionIds = [];
    disconnectConnectionsById.mockResolvedValue(true);
    activeGraphPath = graphPath;
    activeGroupId = 'group-a';
    useGraphDataStore.setState({ graphEntities: {} });
    useLayoutStore.setState({ activeEditorGroupId: activeGroupId });
    useEditorTabStore.setState({
      registry: {
        [graphPath]: { id: graphPath, component: 'GraphEditor', type: 'event' },
      },
      placements: {
        [activeGroupId]: {
          tabIds: [graphPath],
          activeTabId: graphPath,
          selectedNodeIds: [...selectedNodeIds],
          selectedConnectionIds: [...selectedConnectionIds],
          selectedTabIds: [graphPath],
        },
      },
    });
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

  it('breaks all node links with one DisconnectNode command and no connection discovery loop', async () => {
    installNode(true, true);
    await renderOperations();

    await operations.breakAllNodeLinks(nodeId);

    expect(executeCommandOutcome).toHaveBeenCalledTimes(1);
    expect(executeCommandOutcome).toHaveBeenCalledWith(
      graphPath,
      'DisconnectNode',
      { nodeId },
    );
  });

  it('copies stable projection identity without legacy params or display identity', async () => {
    installNode(true, true);
    const storedNode = useGraphDataStore.getState().getGraphNode(graphPath, nodeId);
    if (!storedNode) throw new Error('projected node was not installed');
    Object.assign(storedNode, {
      paramsKind: 'variable',
      variableId: 'variable-id',
      variableName: 'Display variable name',
      variableType: 'Float64',
      subGraphPath: 'functions/display-name',
      dataframeId: 'database-id',
    });
    await renderOperations();

    operations.copy();

    expect(setClipboard).toHaveBeenCalledWith({
      entries: [{
        nodeType: 'tests.projected-node',
        position: { x: 0, y: 0 },
        pins: expect.any(Array),
      }],
      internalConnections: expect.any(Array),
    });
  });

  it('disconnects selected edges with one ordered intent and no node deletion', async () => {
    selectedNodeIds = [];
    selectedConnectionIds = ['edge-b', 'edge-a'];
    await renderOperations();
    useEditorTabStore.getState().setSelectedConnectionIds('group-a', ['edge-b', 'edge-a']);

    await operations.deleteSelected();

    expect(disconnectConnectionsById).toHaveBeenCalledOnce();
    expect(disconnectConnectionsById).toHaveBeenCalledWith(graphPath, ['edge-b', 'edge-a']);
    expect(executeCommandOutcome).not.toHaveBeenCalled();
    expect(updateSelectedConnections).toHaveBeenCalledWith([], 'group-a');
  });

  it('breaks a scoped context-menu edge set with one collection intent', async () => {
    selectedNodeIds = [];
    selectedConnectionIds = ['edge-b', 'edge-a'];
    await renderOperations();
    useEditorTabStore.getState().setSelectedConnectionIds('group-a', ['edge-b', 'edge-a']);

    await operations.breakConnectionsById(['edge-b', 'edge-a'], graphPath, 'group-a');

    expect(disconnectConnectionsById).toHaveBeenCalledOnce();
    expect(disconnectConnectionsById).toHaveBeenCalledWith(graphPath, ['edge-b', 'edge-a']);
    expect(updateSelectedConnections).toHaveBeenCalledWith([], 'group-a');
  });

  it('preserves scoped context-menu edge selection when disconnect fails', async () => {
    selectedNodeIds = [];
    selectedConnectionIds = ['edge-a'];
    disconnectConnectionsById.mockResolvedValue(false);
    await renderOperations();

    await operations.breakConnectionsById(['edge-a'], graphPath, 'group-a');

    expect(disconnectConnectionsById).toHaveBeenCalledOnce();
    expect(updateSelectedConnections).not.toHaveBeenCalled();
  });

  it('clears edge selection after applied resolution when authority is unchanged', async () => {
    selectedNodeIds = [];
    selectedConnectionIds = ['edge-a'];
    await renderOperations();
    useEditorTabStore.getState().setSelectedConnectionIds('group-a', ['edge-a']);

    await operations.deleteSelected();

    expect(updateSelectedConnections).toHaveBeenCalledWith([], 'group-a');
  });

  it('preserves edge selection when store authority changes without rerender before applied resolution', async () => {
    selectedNodeIds = [];
    selectedConnectionIds = ['edge-a'];
    let resolveDisconnect!: (value: boolean) => void;
    disconnectConnectionsById.mockReturnValue(new Promise((resolve) => {
      resolveDisconnect = resolve;
    }));
    await renderOperations();
    useEditorTabStore.getState().setSelectedConnectionIds('group-a', ['edge-a']);

    const deletion = operations.deleteSelected();
    useEditorTabStore.getState().setSelectedConnectionIds('group-a', ['edge-b']);
    resolveDisconnect(true);
    await deletion;

    expect(updateSelectedConnections).not.toHaveBeenCalled();
  });

  it('preserves edge selection when disconnect fails', async () => {
    selectedNodeIds = [];
    selectedConnectionIds = ['edge-a'];
    disconnectConnectionsById.mockResolvedValue(false);
    await renderOperations();

    await operations.deleteSelected();

    expect(updateSelectedConnections).not.toHaveBeenCalled();
  });

  it.each([
    ['graph', () => useEditorTabStore.setState((state) => {
      state.placements['group-a'].activeTabId = 'events/other';
    })],
    ['group', () => useLayoutStore.setState({ activeEditorGroupId: 'group-b' })],
    ['selection', () => useEditorTabStore.getState().setSelectedConnectionIds('group-a', ['edge-a', 'edge-b'])],
  ] as const)('preserves edge selection when the active %s changes before disconnect settles', async (_case, change) => {
    selectedNodeIds = [];
    selectedConnectionIds = ['edge-a'];
    let resolveDisconnect!: (value: boolean) => void;
    disconnectConnectionsById.mockReturnValue(new Promise((resolve) => {
      resolveDisconnect = resolve;
    }));
    await renderOperations();

    const deletion = operations.deleteSelected();
    change();
    await act(async () => root.render(<Harness />));
    resolveDisconnect(true);
    await deletion;

    expect(updateSelectedConnections).not.toHaveBeenCalled();
  });

  it('preserves node selection when store authority changes without rerender before applied resolution', async () => {
    installNode(true, true);
    let resolveOutcome!: (value: { status: 'applied' }) => void;
    executeCommandOutcome.mockReturnValue(new Promise((resolve) => {
      resolveOutcome = resolve;
    }));
    await renderOperations();

    const deletion = operations.deleteSelected();
    useEditorTabStore.getState().setSelectedNodeIds('group-a', ['other-node']);
    resolveOutcome({ status: 'applied' });
    await deletion;

    expect(updateSelected).not.toHaveBeenCalled();
  });

  it('clears the captured group selection only after authoritative applied deletion', async () => {
    installNode(true, true);
    await renderOperations();

    await operations.deleteSelected();

    expect(updateSelected).toHaveBeenCalledOnce();
    expect(updateSelected).toHaveBeenCalledWith([], 'group-a');
  });

  it.each([
    { status: 'noop' },
    { status: 'rejected', code: 'graph_managed_node_delete_forbidden' },
    { status: 'conflict' },
    { status: 'stale' },
  ])('preserves selection when authoritative deletion returns $status', async (outcome) => {
    installNode(true, true);
    executeCommandOutcome.mockResolvedValue(outcome);
    await renderOperations();

    await operations.deleteSelected();

    expect(updateSelected).not.toHaveBeenCalled();
  });

  it('preserves selection when authoritative deletion throws', async () => {
    installNode(true, true);
    executeCommandOutcome.mockRejectedValue(new Error('failed'));
    await renderOperations();

    await operations.deleteSelected();

    expect(updateSelected).not.toHaveBeenCalled();
  });

  it.each([
    ['graph', () => useEditorTabStore.setState((state) => {
      state.placements['group-a'].activeTabId = 'events/other';
    })],
    ['group', () => useLayoutStore.setState({ activeEditorGroupId: 'group-b' })],
    ['selection', () => useEditorTabStore.getState().setSelectedNodeIds('group-a', [nodeId, 'other-node'])],
  ] as const)('preserves selection when the active %s changes before applied deletion settles', async (_case, change) => {
    installNode(true, true);
    let resolveOutcome!: (value: { status: 'applied' }) => void;
    executeCommandOutcome.mockReturnValue(new Promise((resolve) => {
      resolveOutcome = resolve;
    }));
    await renderOperations();

    const deletion = operations.deleteSelected();
    change();
    await act(async () => root.render(<Harness />));
    resolveOutcome({ status: 'applied' });
    await deletion;

    expect(updateSelected).not.toHaveBeenCalled();
  });

  it('preserves deletion and cut for an ordinary projected node', async () => {
    installNode(true, true);
    await renderOperations();

    await operations.deleteSelected();
    await operations.cutNodes([nodeId]);

    expect(executeCommandOutcome).toHaveBeenNthCalledWith(
      1,
      graphPath,
      'DeleteNodes',
      { nodeIds: [nodeId] },
    );
    expect(executeCommandOutcome).toHaveBeenNthCalledWith(
      2,
      graphPath,
      'DeleteNodes',
      { nodeIds: [nodeId] },
    );
  });
});
