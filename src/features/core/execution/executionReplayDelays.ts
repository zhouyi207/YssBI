/** Intentional delays for stepped replay (live execution uses rAF batching, no delays). */
export const EXECUTION_REPLAY_DELAYS_MS: Record<string, number> = {
  executionStart: 80,
  nodeStart: 120,
  nodeComplete: 80,
  nodeError: 400,
  connectionActive: 100,
  executionComplete: 80,
};

export const EXECUTION_REPLAY_DEFAULT_DELAY_MS = 120;
