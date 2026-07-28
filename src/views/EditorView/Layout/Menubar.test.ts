import { describe, expect, it, vi } from 'vitest';
import { buildEditMenuItems } from './Menubar';

describe('Menubar unavailable editor mutations', () => {
  it('keeps Paste visible but disabled without a stable creation capability', () => {
    const paste = vi.fn();
    const items = buildEditMenuItems(
      (key) => key,
      {
        activeTabId: 'events/main.yssbi-event',
        canUndo: false,
        canRedo: false,
      },
      {
        undo: vi.fn(),
        redo: vi.fn(),
        cut: vi.fn(),
        copy: vi.fn(),
        paste,
        deleteSelected: vi.fn(),
      },
    );

    const pasteItem = items.find((item) => item.label === 'menubar.paste');
    expect(pasteItem?.onClick).toBeUndefined();
  });
});
