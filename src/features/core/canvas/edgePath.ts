/** Shared bezier path math for SVG edges and drag-preview updates. */
export function computeEdgePath(
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  startIsInput = false,
): string {
  const dx = Math.abs(x1 - x2);
  const curvature = Math.max(dx * 0.5, 40);
  const dir = startIsInput ? -1 : 1;
  const c1x = x1 + curvature * dir;
  const c1y = y1;
  const c2x = x2 - curvature * dir;
  const c2y = y2;
  return `M ${x1},${y1} C ${c1x},${c1y} ${c2x},${c2y} ${x2},${y2}`;
}
