import { useCallback, useEffect, useRef, useState, forwardRef, useImperativeHandle } from "react";
import { THUMB_MIN_SIZE, TRACK_SIZE } from "@/app/appConfig/default";
import {
  computeHorizontalScrollbarMetrics,
  computeVerticalScrollbarMetrics,
  pageScrollFromTrackClick,
} from "./overlayScrollbar/metrics";
import {
  beginThumbDragSession,
  bindScrollbarThumbDrag,
  withInstantViewportScroll,
} from "./overlayScrollbar/thumbDrag";

type Direction = "vertical" | "horizontal" | "both";

const ARROW_HEIGHT = TRACK_SIZE * 2;
const SCROLL_STEP = 40;
const SCROLL_INTERVAL = 50;
const SCROLL_INITIAL_DELAY = 300;

const THUMB_COLOR = "var(--overlay-scrollbar-thumb)";
const ARROW_STROKE = "var(--overlay-scrollbar-arrow)";

function ArrowButton({
  direction,
  onScroll,
  size,
}: {
  direction: "up" | "down" | "left" | "right";
  onScroll: () => void;
  size: number;
}) {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const stopScroll = useCallback(() => {
    if (timerRef.current) { clearTimeout(timerRef.current); timerRef.current = null; }
    if (intervalRef.current) { clearInterval(intervalRef.current); intervalRef.current = null; }
  }, []);

  const startScroll = useCallback(() => {
    stopScroll();
    onScroll();
    timerRef.current = setTimeout(() => {
      intervalRef.current = setInterval(onScroll, SCROLL_INTERVAL);
    }, SCROLL_INITIAL_DELAY);
  }, [onScroll, stopScroll]);

  useEffect(() => stopScroll, [stopScroll]);

  const isVertical = direction === "up" || direction === "down";
  const w = isVertical ? size : ARROW_HEIGHT;
  const h = isVertical ? ARROW_HEIGHT : size;

  let d: string;
  switch (direction) {
    case "up":    d = `M${size * 0.15} ${ARROW_HEIGHT * 0.65} L${size * 0.5} ${ARROW_HEIGHT * 0.35} L${size * 0.85} ${ARROW_HEIGHT * 0.65}`; break;
    case "down":  d = `M${size * 0.15} ${ARROW_HEIGHT * 0.35} L${size * 0.5} ${ARROW_HEIGHT * 0.65} L${size * 0.85} ${ARROW_HEIGHT * 0.35}`; break;
    case "left":  d = `M${ARROW_HEIGHT * 0.65} ${size * 0.15} L${ARROW_HEIGHT * 0.35} ${size * 0.5} L${ARROW_HEIGHT * 0.65} ${size * 0.85}`; break;
    case "right": d = `M${ARROW_HEIGHT * 0.35} ${size * 0.15} L${ARROW_HEIGHT * 0.65} ${size * 0.5} L${ARROW_HEIGHT * 0.35} ${size * 0.85}`; break;
  }

  return (
    <button
      type="button"
      aria-label={`Scroll ${direction}`}
      className="shrink-0 flex items-center justify-center cursor-pointer transition-opacity duration-100 hover:opacity-100 opacity-60 touch-none"
      style={{ width: w, height: h }}
      onPointerDown={(e) => { e.preventDefault(); e.stopPropagation(); startScroll(); }}
      onPointerUp={stopScroll}
      onPointerLeave={stopScroll}
      onPointerCancel={stopScroll}
      onKeyDown={(e) => {
        if (e.key !== "Enter" && e.key !== " ") return;
        e.preventDefault();
        startScroll();
      }}
      onKeyUp={stopScroll}
    >
      <svg width={w} height={h} viewBox={`0 0 ${w} ${h}`}>
        <path d={d} stroke={ARROW_STROKE} fill="none" strokeWidth={1.2} strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    </button>
  );
}

/**
 * 自定义悬浮滚动条：不占布局空间，悬浮在内容边缘。
 * 隐藏原生滚动条，通过 JS 实现同步的 overlay 滚动条。
 */
export const OverlayScrollbar = forwardRef<
  HTMLDivElement,
  {
    children: React.ReactNode;
    className?: string;
    direction?: Direction;
    onScroll?: (e: React.UIEvent<HTMLDivElement>) => void;
    /** 垂直滚动条顶部偏移（如 sticky 表头高度），使滚动条从表头下方开始 */
    scrollbarOffsetTop?: number;
    /** 水平滚动条左侧偏移（如行号列宽度），使滚动条从行号列右侧开始 */
    scrollbarOffsetLeft?: number;
  }
>(function OverlayScrollbar({ children, className = "", direction = "vertical", onScroll, scrollbarOffsetTop = 0, scrollbarOffsetLeft = 0 }, ref) {
  const containerRef = useRef<HTMLDivElement>(null);
  const viewportRef = useRef<HTMLDivElement>(null);
  const trackVRef = useRef<HTMLDivElement>(null);
  const trackHRef = useRef<HTMLDivElement>(null);
  const thumbVRef = useRef<HTMLDivElement>(null);
  const thumbHRef = useRef<HTMLDivElement>(null);
  const isDraggingRef = useRef({ v: false, h: false });
  const activeDragCleanupRef = useRef<(() => void) | null>(null);
  const [thumbStyle, setThumbStyle] = useState<{
    v?: { height: number; top: number };
    h?: { width: number; left: number };
  }>({});
  const [isVisible, setIsVisible] = useState<{ v?: boolean; h?: boolean }>({});
  const [isHovered, setIsHovered] = useState(false);
  const [isDragging, setIsDragging] = useState({ v: false, h: false });

  useImperativeHandle(ref, () => viewportRef.current!);

  const getVerticalTrackLength = useCallback(() => {
    const container = containerRef.current;
    if (!container) return 0;
    return container.clientHeight - scrollbarOffsetTop - ARROW_HEIGHT * 2;
  }, [scrollbarOffsetTop]);

  const getHorizontalTrackLength = useCallback(() => {
    const container = containerRef.current;
    if (!container) return 0;
    const rightReserved = direction === "both" ? TRACK_SIZE + 4 : 0;
    return container.clientWidth - scrollbarOffsetLeft - rightReserved - ARROW_HEIGHT * 2;
  }, [direction, scrollbarOffsetLeft]);

  const updateThumb = useCallback(() => {
    const el = viewportRef.current;
    if (!el) return;

    const next: typeof thumbStyle = {};
    const visible: typeof isVisible = {};

    if (direction === "vertical" || direction === "both") {
      const metrics = computeVerticalScrollbarMetrics(el, getVerticalTrackLength());
      if (metrics) {
        visible.v = true;
        next.v = { height: metrics.thumbSize, top: metrics.thumbOffset };
      }
    }

    if (direction === "horizontal" || direction === "both") {
      const metrics = computeHorizontalScrollbarMetrics(el, getHorizontalTrackLength());
      if (metrics) {
        visible.h = true;
        next.h = { width: metrics.thumbSize, left: metrics.thumbOffset };
      }
    }

    setThumbStyle(next);
    setIsVisible(visible);
  }, [direction, getHorizontalTrackLength, getVerticalTrackLength]);

  useEffect(() => {
    const el = viewportRef.current;
    const container = containerRef.current;
    if (!el) return;

    updateThumb();

    let rafId: number | null = null;
    const scheduleThumbSync = () => {
      if (isDraggingRef.current.v || isDraggingRef.current.h) return;
      if (document.body.classList.contains("layout-sash-dragging")) return;
      if (rafId !== null) return;
      rafId = requestAnimationFrame(() => {
        rafId = null;
        updateThumb();
      });
    };

    const ro = new ResizeObserver(scheduleThumbSync);
    ro.observe(el);
    if (container) ro.observe(container);

    const mo = new MutationObserver(scheduleThumbSync);
    mo.observe(el, { childList: true, subtree: true });

    el.addEventListener("scroll", scheduleThumbSync, { passive: true });

    return () => {
      if (rafId !== null) cancelAnimationFrame(rafId);
      ro.disconnect();
      mo.disconnect();
      el.removeEventListener("scroll", scheduleThumbSync);
    };
  }, [updateThumb]);

  useEffect(() => () => activeDragCleanupRef.current?.(), []);

  const scrollV = useCallback((delta: number) => {
    const el = viewportRef.current;
    if (!el) return;
    withInstantViewportScroll(el, () => {
      el.scrollTop = Math.max(0, Math.min(el.scrollHeight - el.clientHeight, el.scrollTop + delta));
    });
  }, []);

  const scrollH = useCallback((delta: number) => {
    const el = viewportRef.current;
    if (!el) return;
    withInstantViewportScroll(el, () => {
      el.scrollLeft = Math.max(0, Math.min(el.scrollWidth - el.clientWidth, el.scrollLeft + delta));
    });
  }, []);

  const handleTrackPointerDownV = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      if (e.button !== 0 || e.target !== e.currentTarget) return;
      const el = viewportRef.current;
      if (!el) return;

      const metrics = computeVerticalScrollbarMetrics(el, getVerticalTrackLength());
      if (!metrics) return;

      const rect = e.currentTarget.getBoundingClientRect();
      const nextScroll = pageScrollFromTrackClick(
        e.clientY - rect.top,
        metrics,
        el.scrollTop,
        el.clientHeight,
      );
      if (nextScroll == null) return;
      withInstantViewportScroll(el, () => {
        el.scrollTop = nextScroll;
      });
    },
    [getVerticalTrackLength],
  );

  const handleTrackPointerDownH = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      if (e.button !== 0 || e.target !== e.currentTarget) return;
      const el = viewportRef.current;
      if (!el) return;

      const metrics = computeHorizontalScrollbarMetrics(el, getHorizontalTrackLength());
      if (!metrics) return;

      const rect = e.currentTarget.getBoundingClientRect();
      const nextScroll = pageScrollFromTrackClick(
        e.clientX - rect.left,
        metrics,
        el.scrollLeft,
        el.clientWidth,
      );
      if (nextScroll == null) return;
      withInstantViewportScroll(el, () => {
        el.scrollLeft = nextScroll;
      });
    },
    [getHorizontalTrackLength],
  );

  const handleThumbPointerDownV = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      const thumb = thumbVRef.current;
      const track = trackVRef.current;
      const viewport = viewportRef.current;
      if (!thumb || !track || !viewport) return;

      const metrics = computeVerticalScrollbarMetrics(viewport, getVerticalTrackLength());
      if (!metrics) return;

      const session = beginThumbDragSession("y", e.nativeEvent, metrics);
      if (!session) return;

      e.preventDefault();
      e.stopPropagation();

      activeDragCleanupRef.current?.();
      activeDragCleanupRef.current = bindScrollbarThumbDrag({
        captureHost: track,
        thumbEl: thumb,
        viewport,
        session,
        onDraggingChange: (dragging) => {
          isDraggingRef.current.v = dragging;
          setIsDragging((prev) => ({ ...prev, v: dragging }));
        },
        onDragEnd: () => {
          activeDragCleanupRef.current = null;
          updateThumb();
        },
      });
    },
    [getVerticalTrackLength, updateThumb],
  );

  const handleThumbPointerDownH = useCallback(
    (e: React.PointerEvent<HTMLDivElement>) => {
      const thumb = thumbHRef.current;
      const track = trackHRef.current;
      const viewport = viewportRef.current;
      if (!thumb || !track || !viewport) return;

      const metrics = computeHorizontalScrollbarMetrics(viewport, getHorizontalTrackLength());
      if (!metrics) return;

      const session = beginThumbDragSession("x", e.nativeEvent, metrics);
      if (!session) return;

      e.preventDefault();
      e.stopPropagation();

      activeDragCleanupRef.current?.();
      activeDragCleanupRef.current = bindScrollbarThumbDrag({
        captureHost: track,
        thumbEl: thumb,
        viewport,
        session,
        onDraggingChange: (dragging) => {
          isDraggingRef.current.h = dragging;
          setIsDragging((prev) => ({ ...prev, h: dragging }));
        },
        onDragEnd: () => {
          activeDragCleanupRef.current = null;
          updateThumb();
        },
      });
    },
    [getHorizontalTrackLength, updateThumb],
  );

  const overflowX = direction === "horizontal" ? "auto" : "hidden";
  const overflowY = direction === "vertical" ? "auto" : direction === "both" ? "auto" : "hidden";

  const chromeActiveV = isVisible.v && (isHovered || isDragging.v);
  const chromeActiveH = isVisible.h && (isHovered || isDragging.h);

  return (
    <div
      ref={containerRef}
      className={`relative flex flex-col min-h-0 w-full ${className}`}
      onPointerEnter={() => setIsHovered(true)}
      onPointerLeave={() => setIsHovered(false)}
    >
      <div
        ref={viewportRef}
        className={`overlay-scrollbar-viewport flex-1 min-h-0 w-full overscroll-contain ${
          direction === "horizontal" ? "flex flex-row items-start" : ""
        }`}
        style={{
          overflowX,
          overflowY,
          scrollbarWidth: "none",
          msOverflowStyle: "none",
        }}
        onScroll={onScroll}
      >
        {children}
      </div>

      {(direction === "vertical" || direction === "both") && isVisible.v && (
        <div
          className="absolute right-0 bottom-0 w-6 flex justify-end pointer-events-none transition-opacity duration-150 ease-out z-20"
          style={{ top: scrollbarOffsetTop, paddingRight: 2 }}
        >
          <div
            className={`flex flex-col h-full ${chromeActiveV ? "pointer-events-auto" : ""}`}
            style={{ opacity: chromeActiveV ? 1 : 0, width: TRACK_SIZE }}
          >
            <ArrowButton direction="up" size={TRACK_SIZE} onScroll={() => scrollV(-SCROLL_STEP)} />
            <div
              ref={trackVRef}
              className="flex-1 min-h-0 relative cursor-pointer touch-none"
              onPointerDown={handleTrackPointerDownV}
              style={{ width: TRACK_SIZE }}
            >
              {thumbStyle.v && (
                <div
                  ref={thumbVRef}
                  className="absolute left-0 right-0 cursor-grab active:cursor-grabbing transition-colors duration-150 ease-out hover:bg-[var(--overlay-scrollbar-thumb-hover)] touch-none select-none"
                  style={{
                    top: thumbStyle.v.top,
                    height: thumbStyle.v.height,
                    width: TRACK_SIZE,
                    minHeight: THUMB_MIN_SIZE,
                    backgroundColor: THUMB_COLOR,
                  }}
                  onPointerDown={handleThumbPointerDownV}
                />
              )}
            </div>
            <ArrowButton direction="down" size={TRACK_SIZE} onScroll={() => scrollV(SCROLL_STEP)} />
          </div>
        </div>
      )}

      {(direction === "horizontal" || direction === "both") && isVisible.h && (
        <div
          className="absolute bottom-0 h-6 flex items-end pointer-events-none transition-opacity duration-150 ease-out z-20"
          style={{ left: scrollbarOffsetLeft, paddingBottom: 2, right: direction === "both" ? TRACK_SIZE + 4 : 0 }}
        >
          <div
            className={`flex flex-row w-full ${chromeActiveH ? "pointer-events-auto" : ""}`}
            style={{ opacity: chromeActiveH ? 1 : 0, height: TRACK_SIZE }}
          >
            <ArrowButton direction="left" size={TRACK_SIZE} onScroll={() => scrollH(-SCROLL_STEP)} />
            <div
              ref={trackHRef}
              className="flex-1 min-w-0 relative cursor-pointer touch-none"
              onPointerDown={handleTrackPointerDownH}
              style={{ height: TRACK_SIZE }}
            >
              {thumbStyle.h && (
                <div
                  ref={thumbHRef}
                  className="absolute top-0 bottom-0 cursor-grab active:cursor-grabbing transition-colors duration-150 ease-out hover:bg-[var(--overlay-scrollbar-thumb-hover)] touch-none select-none"
                  style={{
                    left: thumbStyle.h.left,
                    width: thumbStyle.h.width,
                    height: TRACK_SIZE,
                    minWidth: THUMB_MIN_SIZE,
                    backgroundColor: THUMB_COLOR,
                  }}
                  onPointerDown={handleThumbPointerDownH}
                />
              )}
            </div>
            <ArrowButton direction="right" size={TRACK_SIZE} onScroll={() => scrollH(SCROLL_STEP)} />
          </div>
        </div>
      )}
    </div>
  );
});
