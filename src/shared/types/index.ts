export type Position = { x: number; y: number };

export function clamp(v: number, min: number, max: number) {
  return Math.min(max, Math.max(min, v));
}

// Re-export all type modules
export * from './graph';
export * from './settings';
export * from './editor';
export * from './layout';
export * from './loadStatus';
export * from './logging';
