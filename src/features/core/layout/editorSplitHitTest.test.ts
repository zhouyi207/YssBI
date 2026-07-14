import { describe, expect, it } from 'vitest';
import { resolveEditorSplitHit } from './editorSplitHitTest';

const SIZE = { width: 900, height: 600 };

describe('resolveEditorSplitHit', () => {
  it('returns merge in the center dead zone', () => {
    expect(resolveEditorSplitHit(SIZE, 450, 300).mode).toBe('merge');
  });

  it('splits left/right in the outer 10% horizontal bands', () => {
    expect(resolveEditorSplitHit(SIZE, 50, 300)).toEqual({ mode: 'split', edge: 'left' });
    expect(resolveEditorSplitHit(SIZE, 850, 300)).toEqual({ mode: 'split', edge: 'right' });
  });

  it('splits up/down in the outer 10% vertical bands', () => {
    expect(resolveEditorSplitHit(SIZE, 450, 30)).toEqual({ mode: 'split', edge: 'top' });
    expect(resolveEditorSplitHit(SIZE, 450, 570)).toEqual({ mode: 'split', edge: 'bottom' });
  });

  it('merges when splitting is disabled (VS Code Alt toggle)', () => {
    expect(resolveEditorSplitHit(SIZE, 50, 50, { enableSplitting: false }).mode).toBe('merge');
  });
});
