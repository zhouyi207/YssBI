import { describe, expect, it } from 'vitest';
import {
  isEditorDragCopyOperation,
  isEditorDragToggleSplitOperation,
  resolveEnableSplittingOnDrag,
} from './editorDragModifiers';

describe('editorDragModifiers', () => {
  it('toggles split on alt for non-mac (VS Code Windows/Linux)', () => {
    expect(isEditorDragToggleSplitOperation({ altKey: true, shiftKey: false })).toBe(true);
    expect(resolveEnableSplittingOnDrag(true, { altKey: true, shiftKey: false })).toBe(false);
    expect(resolveEnableSplittingOnDrag(false, { altKey: true, shiftKey: false })).toBe(true);
  });

  it('uses ctrl for copy on non-mac', () => {
    expect(isEditorDragCopyOperation({ altKey: false, ctrlKey: true })).toBe(true);
    expect(isEditorDragCopyOperation({ altKey: true, ctrlKey: false })).toBe(false);
  });
});
