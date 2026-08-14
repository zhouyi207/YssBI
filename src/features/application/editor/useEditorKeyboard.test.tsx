// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useEditorKeyboard } from './useEditorKeyboard';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { getCanvasInteraction, useGraphInteractionStore } from '@/features/core/graphInteraction/graphInteractionStore';
import { useEditorStore } from '@/features/core/editor';

const exitZenMode = vi.hoisted(() => vi.fn());
vi.mock('@/features/core/layout/workbenchZenMode', () => ({
  exitZenMode,
  isZenModeActive: () => true,
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const noop = () => {};
const deleteSelected = vi.fn();
const cut = vi.fn();
const paste = vi.fn();
const duplicateSelected = vi.fn();

const baseProps = {
  deleteSelected,
  undo: noop,
  redo: noop,
  copy: noop,
  cut,
  paste,
  duplicateSelected,
  saveGraph: noop,
  saveGraphAs: noop,
  importGraph: noop,
  addEvent: noop,
  closeTab: noop,
  setActiveTabId: noop,
  splitEditorRight: noop,
};

function KeyboardHarness() {
  useEditorKeyboard(baseProps);
  return null;
}

describe('useEditorKeyboard', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    useGraphInteractionStore.setState({ interactions: {}, positionOverrides: {} });
    useLayoutStore.setState({ activeEditorGroupId: 'group-1', zenMode: true });
    useEditorTabStore.setState({
      registry: {
        'events/main.yssbi-event': {
          id: 'events/main.yssbi-event',
          component: 'GraphEditor',
          type: 'event',
        },
      },
      placements: {
        'group-1': {
          tabIds: ['events/main.yssbi-event'],
          activeTabId: 'events/main.yssbi-event',
          selectedNodeIds: ['node-a'],
          selectedConnectionIds: [],
          selectedTabIds: ['events/main.yssbi-event'],
        },
      },
    });
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => {
      root.render(<KeyboardHarness />);
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    host.remove();
    vi.restoreAllMocks();
  });

  it('opens node documentation with F1', () => {
    useLayoutStore.setState({ isNodeDocumentationOpen: false });
    const event = new KeyboardEvent('keydown', { key: 'F1', bubbles: true, cancelable: true });
    const preventDefault = vi.spyOn(event, 'preventDefault');

    window.dispatchEvent(event);

    expect(preventDefault).toHaveBeenCalled();
    expect(useLayoutStore.getState().isNodeDocumentationOpen).toBe(true);
  });

  it.each([
    { key: 'v', action: paste },
    { key: 'd', action: duplicateSelected },
  ])('does not route disabled Ctrl+$key mutation shortcuts', ({ key, action }) => {
    const event = new KeyboardEvent('keydown', {
      key,
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });

    window.dispatchEvent(event);

    expect(action).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(true);
  });

  it('consumes Ctrl+X before routing the graph cut', () => {
    const event = new KeyboardEvent('keydown', {
      key: 'x',
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });

    window.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
    expect(cut).toHaveBeenCalledOnce();
  });

  it('ignores repeated Ctrl+X keydown events', () => {
    const event = new KeyboardEvent('keydown', {
      key: 'x',
      ctrlKey: true,
      repeat: true,
      bubbles: true,
      cancelable: true,
    });

    window.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
    expect(cut).not.toHaveBeenCalled();
  });

  it('cancels a connection interaction before selection and Zen Mode', () => {
    useGraphInteractionStore.getState().startInteraction('events/main.yssbi-event', {
      type: 'drawingConnection',
      session: {
        groupId: 'group-1',
        graphPath: 'events/main.yssbi-event',
        source: {} as never,
        screenX: 0,
        screenY: 0,
        worldX: 0,
        worldY: 0,
        hoveredTarget: null,
        snappedTarget: null,
        snappedWorld: null,
        feedback: null,
      },
    });

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));

    expect(getCanvasInteraction(useGraphInteractionStore.getState(), 'events/main.yssbi-event', 'group-1')).toEqual({ type: 'idle' });
    expect(useEditorTabStore.getState().getPlacement('group-1').selectedNodeIds).toEqual(['node-a']);
    expect(exitZenMode).not.toHaveBeenCalled();
  });

  it('closes pending node creation and palette without clearing selection or Zen Mode', () => {
    useGraphInteractionStore.getState().startInteraction('events/main.yssbi-event', {
      type: 'pendingNodeCreation',
      session: {
        groupId: 'group-1',
        graphPath: 'events/main.yssbi-event',
        source: null,
        screenX: 20,
        screenY: 30,
      },
    });
    useEditorStore.setState({ contextMenu: { x: 20, y: 30, visible: true } });

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));

    expect(getCanvasInteraction(useGraphInteractionStore.getState(), 'events/main.yssbi-event', 'group-1')).toEqual({ type: 'idle' });
    expect(useEditorStore.getState().contextMenu).toBeNull();
    expect(useEditorTabStore.getState().getPlacement('group-1').selectedNodeIds).toEqual(['node-a']);
    expect(exitZenMode).not.toHaveBeenCalled();
  });

  it('clears edge selection before node selection and Zen Mode', () => {
    useEditorTabStore.getState().setSelectedConnectionIds('group-1', ['edge-a']);

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
    expect(useEditorTabStore.getState().getPlacement('group-1').selectedConnectionIds).toEqual([]);
    expect(useEditorTabStore.getState().getPlacement('group-1').selectedNodeIds).toEqual([]);
    expect(exitZenMode).not.toHaveBeenCalled();

    useEditorTabStore.getState().setSelectedNodeIds('group-1', ['node-a']);
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
    expect(useEditorTabStore.getState().getPlacement('group-1').selectedNodeIds).toEqual([]);
    expect(exitZenMode).not.toHaveBeenCalled();

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
    expect(exitZenMode).toHaveBeenCalledOnce();
  });

  it.each(['Delete', 'Backspace'])('delegates %s deletion exactly once', (key) => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));

    expect(deleteSelected).toHaveBeenCalledOnce();
  });

  it('clears node selection before exiting Zen Mode', () => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
    expect(useEditorTabStore.getState().getPlacement('group-1').selectedNodeIds).toEqual([]);
    expect(exitZenMode).not.toHaveBeenCalled();

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
    expect(exitZenMode).toHaveBeenCalledOnce();
  });

  it('does not preventDefault on Alt so native menu access still works', () => {
    const event = new KeyboardEvent('keydown', { key: 'Alt', bubbles: true, cancelable: true });
    const preventDefault = vi.spyOn(event, 'preventDefault');

    window.dispatchEvent(event);

    expect(preventDefault).not.toHaveBeenCalled();
  });
});
