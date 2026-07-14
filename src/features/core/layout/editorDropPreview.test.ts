import { describe, expect, it } from 'vitest';
import { computeEditorSplitPreviewRect } from './editorDropPreview';

const nodeRect = {
  top: 100,
  left: 200,
  width: 400,
  height: 300,
  right: 600,
  bottom: 400,
  x: 200,
  y: 100,
  toJSON: () => ({}),
} as DOMRect;

describe('computeEditorSplitPreviewRect', () => {
  it('highlights full area on merge (center)', () => {
    expect(computeEditorSplitPreviewRect(nodeRect, 'center')).toEqual({
      top: 100,
      left: 200,
      width: 400,
      height: 300,
    });
  });

  it('highlights left half', () => {
    expect(computeEditorSplitPreviewRect(nodeRect, 'left')).toEqual({
      top: 100,
      left: 200,
      width: 200,
      height: 300,
    });
  });

  it('highlights bottom half', () => {
    expect(computeEditorSplitPreviewRect(nodeRect, 'bottom')).toEqual({
      top: 250,
      left: 200,
      width: 400,
      height: 150,
    });
  });
});
