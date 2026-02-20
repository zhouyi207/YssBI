import { useCallback, useEffect, useRef, useState, forwardRef, useImperativeHandle } from "react";

const THUMB_MIN_SIZE = 24;
const TRACK_SIZE = 6;

type Direction = "vertical" | "horizontal" | "both";

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
  }
>(function OverlayScrollbar({ children, className = "", direction = "vertical", onScroll }, ref) {
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
    if (!el) return;

    const next: typeof thumbStyle = {};
    const visible: typeof isVisible = {};

    if (direction === "vertical" || direction === "both") {
      const { scrollHeight, clientHeight, scrollTop } = el;
      const maxScroll = scrollHeight - clientHeight;
      if (maxScroll > 0) {
        visible.v = true;
        const trackHeight = clientHeight;
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
        const trackWidth = clientWidth;
        const thumbWidth = Math.max(THUMB_MIN_SIZE, (clientWidth / scrollWidth) * trackWidth);
        const thumbLeft = (scrollLeft / maxScroll) * (trackWidth - thumbWidth);
        next.h = { width: thumbWidth, left: thumbLeft };
      } else {
        visible.h = false;
      }
    }

    setThumbStyle(next);
    setIsVisible(visible);
  }, [direction]);

  useEffect(() => {
    const el = viewportRef.current;
    if (!el) return;

    updateThumb();

    const ro = new ResizeObserver(updateThumb);
    ro.observe(el);

    const mo = new MutationObserver(updateThumb);
    mo.observe(el, { childList: true, subtree: true });

    el.addEventListener("scroll", updateThumb, { passive: true });

    return () => {
      ro.disconnect();
      mo.disconnect();
      el.removeEventListener("scroll", updateThumb);
    };
  }, [updateThumb]);

  const overflowX = direction === "horizontal" ? "auto" : "hidden";
  const overflowY = direction === "vertical" ? "auto" : direction === "both" ? "auto" : "hidden";

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
      if (!el) return;

      const deltaY = moveE.clientY - dragStart.current.y;
      dragStart.current = { x: moveE.clientX, y: moveE.clientY };

      const { scrollHeight, clientHeight } = el;
      const maxScroll = scrollHeight - clientHeight;
      const trackHeight = clientHeight;
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
  }, []);

  const handleThumbMouseDownH = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragStart.current = { x: e.clientX, y: e.clientY };

    const onMouseMove = (moveE: MouseEvent) => {
      const el = viewportRef.current;
      if (!el) return;

      const deltaX = moveE.clientX - dragStart.current.x;
      dragStart.current = { x: moveE.clientX, y: moveE.clientY };

      const { scrollWidth, clientWidth } = el;
      const maxScroll = scrollWidth - clientWidth;
      const trackWidth = clientWidth;
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
  }, []);

  const showV = isVisible.v && isHovered;
  const showH = isVisible.h && isHovered;

  return (
    <div
      className={`relative h-full min-h-0 w-full ${className}`}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      <div
        ref={viewportRef}
        className={`overlay-scrollbar-viewport h-full w-full overscroll-contain ${
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
          className={`absolute top-0 right-0 bottom-0 w-6 flex justify-end transition-opacity duration-150 ease-out ${
            showV ? "pointer-events-auto" : "pointer-events-none"
          }`}
          style={{ paddingRight: 2 }}
        >
          <div className="flex flex-col" style={{ opacity: showV ? 1 : 0, width: TRACK_SIZE }}>
            <div
              className="flex-1 min-h-0 relative cursor-pointer"
              onClick={handleTrackClickV}
              style={{ width: TRACK_SIZE }}
            >
              {thumbStyle.v && (
                <div
                  className="absolute left-0 right-0 cursor-grab active:cursor-grabbing transition-colors duration-150 ease-out hover:bg-white/20"
                  style={{
                    top: thumbStyle.v.top,
                    height: thumbStyle.v.height,
                    width: TRACK_SIZE,
                    backgroundColor: "rgba(255,255,255,0.08)",
                  }}
                  onMouseDown={handleThumbMouseDownV}
                />
              )}
            </div>
          </div>
        </div>
      )}

      {(direction === "horizontal" || direction === "both") && isVisible.h && (
        <div
          className={`absolute bottom-0 left-0 h-6 flex items-end transition-opacity duration-150 ease-out ${
            showH ? "pointer-events-auto" : "pointer-events-none"
          }`}
          style={{ paddingBottom: 2, right: direction === "both" ? TRACK_SIZE + 4 : 0 }}
        >
          <div className="flex flex-row w-full" style={{ opacity: showH ? 1 : 0, height: TRACK_SIZE }}>
            <div
              className="flex-1 min-w-0 relative cursor-pointer"
              onClick={handleTrackClickH}
              style={{ height: TRACK_SIZE }}
            >
              {thumbStyle.h && (
                <div
                  className="absolute top-0 bottom-0 cursor-grab active:cursor-grabbing transition-colors duration-150 ease-out hover:bg-white/20"
                  style={{
                    left: thumbStyle.h.left,
                    width: thumbStyle.h.width,
                    height: TRACK_SIZE,
                    backgroundColor: "rgba(255,255,255,0.08)",
                  }}
                  onMouseDown={handleThumbMouseDownH}
                />
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
});
