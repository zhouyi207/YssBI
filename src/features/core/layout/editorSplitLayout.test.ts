import { describe, expect, it } from 'vitest';
import { resolveEditorSplitPlacement } from './editorSplitLayout';

describe('resolveEditorSplitPlacement', () => {
  it('maps center to right split', () => {
    expect(resolveEditorSplitPlacement('center')).toEqual({ direction: 'row', isAfter: true });
  });

  it('maps left to row before target', () => {
    expect(resolveEditorSplitPlacement('left')).toEqual({ direction: 'row', isAfter: false });
  });

  it('maps top to col before target', () => {
    expect(resolveEditorSplitPlacement('top')).toEqual({ direction: 'col', isAfter: false });
  });

  it('maps bottom to col after target', () => {
    expect(resolveEditorSplitPlacement('bottom')).toEqual({ direction: 'col', isAfter: true });
  });
});
