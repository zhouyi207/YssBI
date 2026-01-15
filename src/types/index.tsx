export type Position = { x: number; y: number };


export function clamp(v: number, min: number, max: number) {
  return Math.min(max, Math.max(min, v));
}
