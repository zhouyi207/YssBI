import { useEffect, useRef, useState } from 'react';

/** Plot 容器尺寸订阅（ResizeObserver 单点，避免各图表组件重复实现）。 */
export function usePlotContainerSize() {
  const containerRef = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const sync = () => {
      setSize({ width: container.clientWidth, height: container.clientHeight });
    };

    const ro = new ResizeObserver(sync);
    ro.observe(container);
    sync();
    return () => ro.disconnect();
  }, []);

  return { containerRef, size };
}
