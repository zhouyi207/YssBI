import { useCallback, useEffect, useRef, useState, forwardRef, useImperativeHandle } from "react";
import { THUMB_MIN_SIZE, TRACK_SIZE } from "@/app/appConfig/default";

type Direction = "vertical" | "horizontal" | "both";

const ARROW_HEIGHT = TRACK_SIZE * 2;
const SCROLL_STEP = 40;
const SCROLL_INTERVAL = 50;
const SCROLL_INITIAL_DELAY = 300;

const THUMB_COLOR = "rgba(255,255,255,0.24)";
const ARROW_STROKE = "rgba(255,255,255,0.45)";

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
    onScroll();
    timerRef.current = setTimeout(() => {
      intervalRef.current = setInterval(onScroll, SCROLL_INTERVAL);
    }, SCROLL_INITIAL_DELAY);
  }, [onScroll]);

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
    <div
      className="shrink-0 flex items-center justify-center cursor-pointer transition-opacity duration-100 hover:opacity-100 opacity-60"
      style={{ width: w, height: h }}
      onMouseDown={(e) => { e.preventDefault(); e.stopPropagation(); startScroll(); }}
      onMouseUp={stopScroll}
      onMouseLeave={stopScroll}
    >
      <svg width={w} height={h} viewBox={`0 0 ${w} ${h}`}>
        <path d={d} stroke={ARROW_STROKE} fill="none" strokeWidth={1.2} strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    </div>
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
  const [thumbStyle, setThumbStyle] = useState<{
    v?: { height: number; top: number };
    h?: { width: number; left: number };
  }>({});
  const [isVisible, setIsVisible] = useState<{ v?: boolean; h?: boolean }>({});
  const [isHovered, setIsHovered] = useState(false);
  const dragStart = useRef({ x: 0, y: 0 });

  useImperativeHandle(ref, () => viewportRef.current!);

  const updateThumb = useCallback(() => {
    const el = viewportRef.current;
    const container = containerRef.current;
    if (!el || !container) return;

    const next: typeof thumbStyle = {};
    const visible: typeof isVisible = {};

    if (direction === "vertical" || direction === "both") {
      const { scrollHeight, clientHeight, scrollTop } = el;
      const maxScroll = scrollHeight - clientHeight;
      if (maxScroll > 0) {
        visible.v = true;
        const vHeight = container.clientHeight - scrollbarOffsetTop;
        const trackHeight = vHeight - ARROW_HEIGHT * 2;
        const thumbHeight = Math.max(THUMB_MIN_SIZE, (clientHeight / scrollHeight) * trackHeight);
        const thumbTop = (scrollTop / maxScroll) * (trackHeight - thumbHeight);
        next.v = { height: thumbHeight, top: thumbTop };
      } else {
        visible.v = false;
      }
    }

    if (direction === "horizontal" || direction === "both") {
      const { scrollWidth, clientWidth, scrollLeft } = el;
      const maxScroll = scrollWidth - clientWidth;
      if (maxScroll > 0) {
        visible.h = true;
        const rightReserved = direction === "both" ? TRACK_SIZE + 4 : 0;
        const trackWidth = container.clientWidth - scrollbarOffsetLeft - rightReserved - ARROW_HEIGHT * 2;
        const thumbWidth = Math.max(THUMB_MIN_SIZE, (clientWidth / scrollWidth) * trackWidth);
        const thumbLeft = (scrollLeft / maxScroll) * (trackWidth - thumbWidth);
        next.h = { width: thumbWidth, left: thumbLeft };
      } else {
        visible.h = false;
      }
    }

    setThumbStyle(next);
    setIsVisible(visible);
  }, [direction, scrollbarOffsetTop, scrollbarOffsetLeft]);

  useEffect(() => {
    const el = viewportRef.current;
    if (!el) return;

    updateThumb();

    let rafId: number | null = null;
    const throttledUpdate = () => {
      if (rafId !== null) return;
      rafId = requestAnimationFrame(() => {
        rafId = null;
        updateThumb();
      });
    };

    const ro = new ResizeObserver(throttledUpdate);
    ro.observe(el);

    const mo = new MutationObserver(throttledUpdate);
    mo.observe(el, { childList: true, subtree: true });

    el.addEventListener("scroll", throttledUpdate, { passive: true });

    return () => {
      if (rafId !== null) cancelAnimationFrame(rafId);
      ro.disconnect();
      mo.disconnect();
      el.removeEventListener("scroll", throttledUpdate);
    };
  }, [updateThumb]);

  const overflowX = direction === "horizontal" ? "auto" : "hidden";
  const overflowY = direction === "vertical" ? "auto" : direction === "both" ? "auto" : "hidden";

  const scrollV = useCallback((delta: number) => {
    const el = viewportRef.current;
    if (!el) return;
    el.scrollTop = Math.max(0, Math.min(el.scrollHeight - el.clientHeight, el.scrollTop + delta));
  }, []);

  const scrollH = useCallback((delta: number) => {
    const el = viewportRef.current;
    if (!el) return;
    el.scrollLeft = Math.max(0, Math.min(el.scrollWidth - el.clientWidth, el.scrollLeft + delta));
  }, []);

  const handleTrackClickV = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      const el = viewportRef.current;
      if (!el || !thumbStyle.v) return;

      const track = e.currentTarget;
      const rect = track.getBoundingClientRect();
      const clickY = e.clientY - rect.top;
      const trackHeight = rect.height;
      const { scrollHeight, clientHeight } = el;
      const maxScroll = scrollHeight - clientHeight;

      const thumbHeight = Math.max(THUMB_MIN_SIZE, (clientHeight / scrollHeight) * trackHeight);
      const thumbTop = (el.scrollTop / maxScroll) * (trackHeight - thumbHeight);

      if (clickY < thumbTop) {
        el.scrollTop = Math.max(0, el.scrollTop - clientHeight);
      } else if (clickY > thumbTop + thumbHeight) {
        el.scrollTop = Math.min(maxScroll, el.scrollTop + clientHeight);
      }
    },
    [thumbStyle.v]
  );

  const handleTrackClickH = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      const el = viewportRef.current;
      if (!el || !thumbStyle.h) return;

      const track = e.currentTarget;
      const rect = track.getBoundingClientRect();
      const clickX = e.clientX - rect.left;
      const trackWidth = rect.width;
      const { scrollWidth, clientWidth } = el;
      const maxScroll = scrollWidth - clientWidth;

      const thumbWidth = Math.max(THUMB_MIN_SIZE, (clientWidth / scrollWidth) * trackWidth);
      const thumbLeft = (el.scrollLeft / maxScroll) * (trackWidth - thumbWidth);

      if (clickX < thumbLeft) {
        el.scrollLeft = Math.max(0, el.scrollLeft - clientWidth);
      } else if (clickX > thumbLeft + thumbWidth) {
        el.scrollLeft = Math.min(maxScroll, el.scrollLeft + clientWidth);
      }
    },
    [thumbStyle.h]
  );

  const handleThumbMouseDownV = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragStart.current = { x: e.clientX, y: e.clientY };

    const onMouseMove = (moveE: MouseEvent) => {
      const el = viewportRef.current;
      const container = containerRef.current;
      if (!el || !container) return;

      const deltaY = moveE.clientY - dragStart.current.y;
      dragStart.current = { x: moveE.clientX, y: moveE.clientY };

      const { scrollHeight, clientHeight } = el;
      const maxScroll = scrollHeight - clientHeight;
      const vHeight = container.clientHeight - scrollbarOffsetTop;
      const trackHeight = vHeight - ARROW_HEIGHT * 2;
      const thumbHeight = Math.max(THUMB_MIN_SIZE, (clientHeight / scrollHeight) * trackHeight);
      const trackThumbSpace = trackHeight - thumbHeight;

      if (trackThumbSpace <= 0) return;

      const scrollDelta = deltaY * (maxScroll / trackThumbSpace);
      el.scrollTop = Math.max(0, Math.min(maxScroll, el.scrollTop + scrollDelta));
    };

    const onMouseUp = () => {
      document.removeEventListener("mousemove", onMouseMove);
      document.removeEventListener("mouseup", onMouseUp);
    };

    document.addEventListener("mousemove", onMouseMove);
    document.addEventListener("mouseup", onMouseUp);
  }, [scrollbarOffsetTop]);

  const handleThumbMouseDownH = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragStart.current = { x: e.clientX, y: e.clientY };

    const onMouseMove = (moveE: MouseEvent) => {
      const el = viewportRef.current;
      const container = containerRef.current;
      if (!el || !container) return;

      const deltaX = moveE.clientX - dragStart.current.x;
      dragStart.current = { x: moveE.clientX, y: moveE.clientY };

      const { scrollWidth, clientWidth } = el;
      const maxScroll = scrollWidth - clientWidth;
      const rightReserved = direction === "both" ? TRACK_SIZE + 4 : 0;
      const trackWidth = container.clientWidth - scrollbarOffsetLeft - rightReserved - ARROW_HEIGHT * 2;
      const thumbWidth = Math.max(THUMB_MIN_SIZE, (clientWidth / scrollWidth) * trackWidth);
      const trackThumbSpace = trackWidth - thumbWidth;

      if (trackThumbSpace <= 0) return;

      const scrollDelta = deltaX * (maxScroll / trackThumbSpace);
      el.scrollLeft = Math.max(0, Math.min(maxScroll, el.scrollLeft + scrollDelta));
    };

    const onMouseUp = () => {
      document.removeEventListener("mousemove", onMouseMove);
      document.removeEventListener("mouseup", onMouseUp);
    };

    document.addEventListener("mousemove", onMouseMove);
    document.addEventListener("mouseup", onMouseUp);
  }, [direction, scrollbarOffsetLeft]);

  const showV = isVisible.v && isHovered;
  const showH = isVisible.h && isHovered;

  return (
    <div
      ref={containerRef}
      className={`relative flex flex-col min-h-0 w-full ${className}`}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
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

      {/* Vertical scrollbar - 从 scrollbarOffsetTop 开始，避开 sticky 表头 */}
      {(direction === "vertical" || direction === "both") && isVisible.v && (
        <div
          className="absolute right-0 bottom-0 w-6 flex justify-end pointer-events-none transition-opacity duration-150 ease-out z-20"
          style={{ top: scrollbarOffsetTop, paddingRight: 2 }}
        >
          <div className={`flex flex-col h-full ${showV ? "pointer-events-auto" : ""}`} style={{ opacity: showV ? 1 : 0, width: TRACK_SIZE }}>
            <ArrowButton direction="up" size={TRACK_SIZE} onScroll={() => scrollV(-SCROLL_STEP)} />
            <div
              className="flex-1 min-h-0 relative cursor-pointer"
              onClick={handleTrackClickV}
              style={{ width: TRACK_SIZE }}
            >
              {thumbStyle.v && (
                <div
                  className="absolute left-0 right-0 cursor-grab active:cursor-grabbing transition-colors duration-150 ease-out hover:bg-white/40"
                  style={{
                    top: thumbStyle.v.top,
                    height: thumbStyle.v.height,
                    width: TRACK_SIZE,
                    backgroundColor: THUMB_COLOR,
                  }}
                  onMouseDown={handleThumbMouseDownV}
                />
              )}
            </div>
            <ArrowButton direction="down" size={TRACK_SIZE} onScroll={() => scrollV(SCROLL_STEP)} />
          </div>
        </div>
      )}

      {/* Horizontal scrollbar - 从 scrollbarOffsetLeft 开始，避开行号列 */}
      {(direction === "horizontal" || direction === "both") && isVisible.h && (
        <div
          className="absolute bottom-0 h-6 flex items-end pointer-events-none transition-opacity duration-150 ease-out z-20"
          style={{ left: scrollbarOffsetLeft, paddingBottom: 2, right: direction === "both" ? TRACK_SIZE + 4 : 0 }}
        >
          <div className={`flex flex-row w-full ${showH ? "pointer-events-auto" : ""}`} style={{ opacity: showH ? 1 : 0, height: TRACK_SIZE }}>
            <ArrowButton direction="left" size={TRACK_SIZE} onScroll={() => scrollH(-SCROLL_STEP)} />
            <div
              className="flex-1 min-w-0 relative cursor-pointer"
              onClick={handleTrackClickH}
              style={{ height: TRACK_SIZE }}
            >
              {thumbStyle.h && (
                <div
                  className="absolute top-0 bottom-0 cursor-grab active:cursor-grabbing transition-colors duration-150 ease-out hover:bg-white/40"
                  style={{
                    left: thumbStyle.h.left,
                    width: thumbStyle.h.width,
                    height: TRACK_SIZE,
                    backgroundColor: THUMB_COLOR,
                  }}
                  onMouseDown={handleThumbMouseDownH}
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
