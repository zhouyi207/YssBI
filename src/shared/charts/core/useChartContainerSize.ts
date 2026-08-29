import { useEffect, useRef, useState, type RefObject } from 'react';
import type { ChartSize } from './types';

export function useChartContainerSize(): {
  containerRef: RefObject<HTMLDivElement | null>;
  size: ChartSize;
} {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [size, setSize] = useState<ChartSize>({ width: 0, height: 0 });
  const lastSizeRef = useRef(size);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    let pendingFrame: number | null = null;
    const measure = () => {
      pendingFrame = null;
      const width = container.clientWidth;
      const height = container.clientHeight;
      const lastSize = lastSizeRef.current;
      if (lastSize.width === width && lastSize.height === height) return;
      const nextSize = { width, height };
      lastSizeRef.current = nextSize;
      setSize(nextSize);
    };
    const scheduleMeasure = () => {
      if (pendingFrame != null) return;
      pendingFrame = requestAnimationFrame(measure);
    };

    const observer = new ResizeObserver(scheduleMeasure);
    observer.observe(container);
    scheduleMeasure();

    return () => {
      observer.disconnect();
      if (pendingFrame != null) cancelAnimationFrame(pendingFrame);
    };
  }, []);

  return { containerRef, size };
}
