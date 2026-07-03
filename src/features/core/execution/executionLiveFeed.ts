import type { ExecutionEvent } from '@/shared/types/ui/execution';
import { applyExecutionVisualEvent } from './executionVisualSession';

type SideEffectHandler = (graphId: string, event: ExecutionEvent) => void;

const IMMEDIATE_EVENTS = new Set<ExecutionEvent['event']>(['pinResultReady', 'openSourceWindow']);

let rafId: number | null = null;
const pendingByGraph = new Map<string, ExecutionEvent[]>();

function flushLiveEvents(): void {
  rafId = null;
  for (const [graphId, events] of pendingByGraph) {
    for (const event of events) {
      applyExecutionVisualEvent(graphId, event);
    }
  }
  pendingByGraph.clear();
}

function scheduleFlush(): void {
  if (rafId !== null) return;
  rafId = requestAnimationFrame(flushLiveEvents);
}

/** Batch visual events per frame; side-effect events (pin results, windows) run immediately. */
export function enqueueLiveExecutionEvent(
  graphId: string,
  event: ExecutionEvent,
  onSideEffect?: SideEffectHandler,
): void {
  if (IMMEDIATE_EVENTS.has(event.event)) {
    onSideEffect?.(graphId, event);
    return;
  }

  const queue = pendingByGraph.get(graphId) ?? [];
  queue.push(event);
  pendingByGraph.set(graphId, queue);
  scheduleFlush();
}

export function flushLiveExecutionEventsNow(): void {
  if (rafId !== null) {
    cancelAnimationFrame(rafId);
    rafId = null;
  }
  flushLiveEvents();
}
