import type { ExecutionEvent } from '@/shared/types/ui/execution';
import { applyExecutionVisualEvent } from './executionVisualSession';

type SideEffectHandler = (graphPath: string, event: ExecutionEvent) => void;

const IMMEDIATE_EVENTS = new Set<ExecutionEvent['event']>(['pinResultReady', 'openSourceWindow']);

let rafId: number | null = null;
const pendingByGraph = new Map<string, ExecutionEvent[]>();

function flushLiveEvents(): void {
  rafId = null;
  const deferredFlows: Array<{ graphPath: string; event: ExecutionEvent }> = [];

  for (const [graphPath, events] of pendingByGraph) {
    for (const event of events) {
      if (event.event === 'connectionFlow') {
        deferredFlows.push({ graphPath, event });
        continue;
      }
      applyExecutionVisualEvent(graphPath, event);
    }
  }
  pendingByGraph.clear();

  if (deferredFlows.length > 0) {
    requestAnimationFrame(() => {
      for (const { graphPath, event } of deferredFlows) {
        applyExecutionVisualEvent(graphPath, event);
      }
    });
  }
}

function scheduleFlush(): void {
  if (rafId !== null) return;
  rafId = requestAnimationFrame(flushLiveEvents);
}

/** Batch visual events per frame; side-effect events (pin results, windows) run immediately. */
export function enqueueLiveExecutionEvent(
  graphPath: string,
  event: ExecutionEvent,
  onSideEffect?: SideEffectHandler,
): void {
  if (IMMEDIATE_EVENTS.has(event.event)) {
    onSideEffect?.(graphPath, event);
    return;
  }

  const queue = pendingByGraph.get(graphPath) ?? [];
  queue.push(event);
  pendingByGraph.set(graphPath, queue);
  scheduleFlush();
}

export function flushLiveExecutionEventsNow(): void {
  if (rafId !== null) {
    cancelAnimationFrame(rafId);
    rafId = null;
  }
  flushLiveEvents();
}
