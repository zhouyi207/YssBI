// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useEditorKeyboard } from './useEditorKeyboard';
import { useLayoutStore } from '@/features/core/layout/layoutStore';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const noop = () => {};
const cut = vi.fn();
const paste = vi.fn();
const duplicateSelected = vi.fn();

const baseProps = {
  deleteSelected: noop,
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

  it('does not preventDefault on Alt so native menu access still works', () => {
    const event = new KeyboardEvent('keydown', { key: 'Alt', bubbles: true, cancelable: true });
    const preventDefault = vi.spyOn(event, 'preventDefault');

    window.dispatchEvent(event);

    expect(preventDefault).not.toHaveBeenCalled();
  });
});
