import { useCallback, useLayoutEffect, useMemo, type RefCallback } from "react";

/** Share committed geometry with this edge's mounted SVG paths. */
export function useEdgePath(path: string): RefCallback<SVGPathElement> {
  const geometry = useMemo(() => ({ path: "", elements: new Set<SVGPathElement>() }), []);
  const pathRef = useCallback<RefCallback<SVGPathElement>>(
    (element) => {
      if (!element) return;
      geometry.elements.add(element);
      element.setAttribute("d", geometry.path);
      return () => {
        geometry.elements.delete(element);
      };
    },
    [geometry],
  );
  // Late-mounted hover/run paths start at the last committed geometry. Coordinate
  // updates reach all mounted paths in this commit, before paint, without a frame queue.
  useLayoutEffect(() => {
    geometry.path = path;
    for (const element of geometry.elements) element.setAttribute("d", path);
  }, [geometry, path]);
  return pathRef;
}
