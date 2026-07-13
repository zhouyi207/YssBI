import { describe, expect, it } from 'vitest';
import {
  pageScrollFromTrackClick,
  scrollOffsetFromThumbPosition,
  thumbOffsetFromPointerDrag,
  type ScrollbarAxisMetrics,
} from './metrics';

const metrics: ScrollbarAxisMetrics = {
  maxScroll: 1000,
  trackLength: 200,
  thumbSize: 40,
  thumbOffset: 80,
  scrollRatio: 1000 / 160,
};

describe('overlayScrollbar metrics', () => {
  it('maps pointer delta to thumb offset', () => {
    expect(thumbOffsetFromPointerDrag(80, 100, 116, 160)).toBe(96);
  });

  it('clamps thumb offset to track travel', () => {
    expect(thumbOffsetFromPointerDrag(150, 0, 200, 160)).toBe(160);
    expect(thumbOffsetFromPointerDrag(0, 0, -200, 160)).toBe(0);
  });

  it('maps thumb offset to scroll position', () => {
    expect(scrollOffsetFromThumbPosition(80, metrics)).toBe(500);
  });

  it('pages scroll when track is clicked outside the thumb', () => {
    expect(pageScrollFromTrackClick(10, metrics, 500, 300)).toBe(200);
    expect(pageScrollFromTrackClick(200, metrics, 500, 300)).toBe(800);
    expect(pageScrollFromTrackClick(100, metrics, 500, 300)).toBeNull();
  });
});
