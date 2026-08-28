import type { ExecutionEvent } from '@/shared/types/ui/execution';
import { applyExecutionVisualEvent } from './executionVisualSession';

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

/** Batch visual execution events per frame. */
export function enqueueLiveExecutionEvent(
  graphPath: string,
  event: ExecutionEvent,
): void {
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
