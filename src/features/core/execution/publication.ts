import type { PinHistoryProjection } from '@/shared/types/ui/execution';
import type { RunEvent, RunOutputChannelEvent } from '@/shared/types/dto/runEvent';

export interface ExecutionProjectionPublication {
  readonly startRun: (graphPath: string, runId: string) => void;
  readonly applyRunEvent: (event: RunEvent) => void;
  readonly applyRunOutput: (event: RunOutputChannelEvent) => void;
  readonly applyPinHistory: (projection: PinHistoryProjection) => void;
  readonly finishRun: (graphPath: string) => void;
  readonly clearForProject: (projectInstanceId: string | null) => void;
}
