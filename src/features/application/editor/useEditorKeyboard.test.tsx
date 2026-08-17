// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useEditorKeyboard } from './useEditorKeyboard';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  modalOpen: false,
  history: { canUndo: false, canRedo: false, pending: false },
  setModifierKeys: vi.fn(),
  resetModifierKeys: vi.fn(),
  commands: {
    deleteSelected: vi.fn(),
    undo: vi.fn(),
    redo: vi.fn(),
    copy: vi.fn(),
    cut: vi.fn(),
    paste: vi.fn(),
    duplicateSelected: vi.fn(),
    saveGraph: vi.fn(),
    saveGraphAs: vi.fn(),
    importGraph: vi.fn(),
    addEvent: vi.fn(),
    closeTab: vi.fn(),
    setActiveTabId: vi.fn(),
    splitEditorRight: vi.fn(),
    selectAllNodes: vi.fn(() => true),
    focusSelectedNodes: vi.fn(() => true),
    fitCompleteGraph: vi.fn(() => true),
  },
}));

vi.mock('@/features/core/keyboard', () => ({
  isAppModalOpen: () => mocks.modalOpen,
  useModifierKeyStore: { getState: () => ({
    setModifierKeys: mocks.setModifierKeys,
    resetModifierKeys: mocks.resetModifierKeys,
  }) },
}));
vi.mock('@/features/core/history', () => ({
  useHistoryStore: Object.assign(
    (selector: (state: typeof mocks.history) => unknown) => selector(mocks.history),
    { getState: () => mocks.history },
  ),
}));
vi.mock('@/features/core/dockview', () => ({
  editorDockviewPort: {
    getActiveGroupId: () => 'group-a',
    getActivePanel: () => null,
  },
}));
vi.mock('@/features/core/layout/layoutTabQueries', () => ({
  clearEditorGroupGraphSelection: vi.fn(),
  getActiveLayoutTab: () => null,
  getEditorGroupGraphSelection: () => ({ nodeIds: new Set(), connectionIds: new Set() }),
  resolveEditorTargetGroupId: () => 'group-a',
}));
vi.mock('@/features/core/viewport', () => ({
  getViewport: () => ({ x: 0, y: 0, scale: 1 }),
  editorViewportScope: () => ({ groupId: 'group-a', graphPath: 'events/main.yssbi-event' }),
}));
vi.mock('@/features/core/layout/workbenchZenMode', () => ({
  exitZenMode: vi.fn(),
  isZenModeActive: () => false,
  toggleZenMode: vi.fn(),
}));
vi.mock('@/features/core/layout/workbenchLayoutService', () => ({
  toggleDetailVisibility: vi.fn(),
  togglePanelCollapsed: vi.fn(),
  toggleSidebarVisibility: vi.fn(),
}));
vi.mock('@/features/core/workbench', () => ({
  useWorkbenchStore: { getState: () => ({ setNodeDocumentationOpen: vi.fn() }) },
}));
vi.mock('@/features/core/graphInteraction/graphInteractionStore', () => ({
  getCanvasInteraction: () => ({ type: 'idle' }),
  useGraphInteractionStore: { getState: () => ({}) },
}));
vi.mock('@/features/core/canvas/canvasInteractionCleanup', () => ({ cancelCanvasInteraction: vi.fn() }));
vi.mock('@/features/core/editor', () => ({
  useEditorStore: { getState: () => ({ setContextMenu: vi.fn() }) },
}));
vi.mock('./dockviewTabProjection', () => ({ listDockviewGroupTabs: () => [] }));
vi.mock('./EditorSessionContext', () => ({
  useEditorSessionCommandsContext: () => mocks.commands,
}));

const callbacks = mocks.commands;

let root: Root;

function Harness() {
  useEditorKeyboard();
  return null;
}

function keydown(key: string, init: KeyboardEventInit = {}): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init });
  window.dispatchEvent(event);
  return event;
}

describe('useEditorKeyboard graph canvas shortcuts', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.modalOpen = false;
    document.body.replaceChildren();
    root = createRoot(document.createElement('div'));
    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
  });

  it.each([{ ctrlKey: true }, { metaKey: true }])('routes Ctrl/Meta+A and prevents default', (modifier) => {
    const event = keydown('a', modifier);
    expect(callbacks.selectAllNodes).toHaveBeenCalledOnce();
    expect(event.defaultPrevented).toBe(true);
  });

  it('routes only plain F and plain Home', () => {
    const focusEvent = keydown('f');
    const homeEvent = keydown('Home');
    const modifiedFocusEvent = keydown('f', { ctrlKey: true });
    const modifiedHomeEvent = keydown('Home', { shiftKey: true });

    expect(callbacks.focusSelectedNodes).toHaveBeenCalledOnce();
    expect(callbacks.fitCompleteGraph).toHaveBeenCalledOnce();
    expect(focusEvent.defaultPrevented).toBe(true);
    expect(homeEvent.defaultPrevented).toBe(true);
    expect(modifiedFocusEvent.defaultPrevented).toBe(false);
    expect(modifiedHomeEvent.defaultPrevented).toBe(false);
  });

  it.each([
    ['Ctrl+A', 'a', { ctrlKey: true }, callbacks.selectAllNodes],
    ['F', 'f', {}, callbacks.focusSelectedNodes],
    ['Home', 'Home', {}, callbacks.fitCompleteGraph],
  ] as const)('does not prevent default when %s is a command no-op', (_label, key, init, callback) => {
    callback.mockReturnValueOnce(false);
    const event = keydown(key, init);
    expect(callback).toHaveBeenCalledOnce();
    expect(event.defaultPrevented).toBe(false);
  });

  it.each([
    ['Ctrl+A', 'a', { ctrlKey: true, repeat: true }, callbacks.selectAllNodes],
    ['F', 'f', { repeat: true }, callbacks.focusSelectedNodes],
    ['Home', 'Home', { repeat: true }, callbacks.fitCompleteGraph],
  ] as const)('ignores repeated %s keydown', (_label, key, init, callback) => {
    const event = keydown(key, init);
    expect(callback).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(false);
  });

  it('suppresses graph shortcuts in inputs and contenteditable elements', () => {
    const input = document.createElement('input');
    document.body.appendChild(input);
    input.focus();
    keydown('a', { ctrlKey: true });
    keydown('f');

    const editable = document.createElement('div');
    editable.contentEditable = 'true';
    document.body.appendChild(editable);
    editable.focus();
    keydown('Home');

    expect(callbacks.selectAllNodes).not.toHaveBeenCalled();
    expect(callbacks.focusSelectedNodes).not.toHaveBeenCalled();
    expect(callbacks.fitCompleteGraph).not.toHaveBeenCalled();
  });

  it('suppresses graph shortcuts while a modal is open', () => {
    mocks.modalOpen = true;
    keydown('a', { metaKey: true });
    keydown('f');
    keydown('Home');

    expect(callbacks.selectAllNodes).not.toHaveBeenCalled();
    expect(callbacks.focusSelectedNodes).not.toHaveBeenCalled();
    expect(callbacks.fitCompleteGraph).not.toHaveBeenCalled();
  });
});
