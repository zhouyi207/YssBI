import { THUMB_MIN_SIZE } from '@/app/appConfig/default';

export interface ScrollbarAxisMetrics {
  maxScroll: number;
  trackLength: number;
  thumbSize: number;
  thumbOffset: number;
  /** Pixels of viewport scroll per pixel of thumb travel along the track. */
  scrollRatio: number;
}

function buildMetrics(
  scrollSize: number,
  clientSize: number,
  scrollOffset: number,
  trackLength: number,
): ScrollbarAxisMetrics | null {
  const maxScroll = scrollSize - clientSize;
  if (maxScroll <= 0 || trackLength <= 0) return null;

  const thumbSize = Math.max(THUMB_MIN_SIZE, (clientSize / scrollSize) * trackLength);
  const travel = trackLength - thumbSize;
  const thumbOffset = travel > 0 ? (scrollOffset / maxScroll) * travel : 0;

  return {
    maxScroll,
    trackLength,
    thumbSize,
    thumbOffset,
    scrollRatio: travel > 0 ? maxScroll / travel : 0,
  };
}

export function computeVerticalScrollbarMetrics(
  viewport: HTMLElement,
  trackLength: number,
): ScrollbarAxisMetrics | null {
  const { scrollHeight, clientHeight, scrollTop } = viewport;
  return buildMetrics(scrollHeight, clientHeight, scrollTop, trackLength);
}

export function computeHorizontalScrollbarMetrics(
  viewport: HTMLElement,
  trackLength: number,
): ScrollbarAxisMetrics | null {
  const { scrollWidth, clientWidth, scrollLeft } = viewport;
  return buildMetrics(scrollWidth, clientWidth, scrollLeft, trackLength);
}

export function thumbTravel(metrics: ScrollbarAxisMetrics): number {
  return Math.max(0, metrics.trackLength - metrics.thumbSize);
}

export function thumbOffsetFromPointerDrag(
  originThumbOffset: number,
  originPointer: number,
  currentPointer: number,
  travel: number,
): number {
  const next = originThumbOffset + (currentPointer - originPointer);
  return Math.max(0, Math.min(travel, next));
}

export function scrollOffsetFromThumbPosition(
  thumbOffset: number,
  metrics: ScrollbarAxisMetrics,
): number {
  const travel = thumbTravel(metrics);
  if (travel <= 0) return 0;
  return (thumbOffset / travel) * metrics.maxScroll;
}

export function pageScrollFromTrackClick(
  clickOffset: number,
  metrics: ScrollbarAxisMetrics,
  currentScroll: number,
  pageSize: number,
): number | null {
  const { thumbOffset, thumbSize, maxScroll } = metrics;
  if (clickOffset < thumbOffset) {
    return Math.max(0, currentScroll - pageSize);
  }
  if (clickOffset > thumbOffset + thumbSize) {
    return Math.min(maxScroll, currentScroll + pageSize);
  }
  return null;
}
