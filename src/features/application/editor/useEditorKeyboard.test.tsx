// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useEditorKeyboard } from './useEditorKeyboard';

const noop = () => {};

const baseProps = {
  deleteSelected: noop,
  undo: noop,
  redo: noop,
  copy: noop,
  cut: noop,
  paste: noop,
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

  it('does not preventDefault on Alt so native menu access still works', () => {
    const event = new KeyboardEvent('keydown', { key: 'Alt', bubbles: true, cancelable: true });
    const preventDefault = vi.spyOn(event, 'preventDefault');

    window.dispatchEvent(event);

    expect(preventDefault).not.toHaveBeenCalled();
  });
});
