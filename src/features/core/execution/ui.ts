import type { RecordedEvent } from '@/shared/types/ui/execution';
import type { PinHistoryProjection } from '@/shared/types/ui/execution';
import type { PortAddressDto } from '@/shared/types/domain/editorProjection';
import { pinHistoryCacheKey } from './pinResultIndex';

export interface ExecutionUi {
  readonly setRecording: (graphPath: string, recording: readonly RecordedEvent[]) => void;
  readonly setPlaying: (playing: boolean, graphPath?: string) => void;
  readonly resetVisuals: (graphPath: string) => void;
}

import { useExecutionStore } from './useExecutionStore';

export const executionUi: ExecutionUi = {
  setRecording: (graphPath, recording) =>
    useExecutionStore.getState().setRecording(graphPath, [...recording]),
  setPlaying: (playing, graphPath) => useExecutionStore.getState().setPlaying(playing, graphPath),
  resetVisuals: (graphPath) => useExecutionStore.getState().resetGraphVisuals(graphPath),
};

export interface ExecutionResultUi {
  readonly recordPinHistory: (projection: import('@/shared/types/ui/execution').PinHistoryProjection) => void;
  readonly getPinHistory: (
    graphPath: string,
    output: PortAddressDto,
  ) => PinHistoryProjection | undefined;
  readonly clearRunOutput: (graphPath: string) => void;
}

export const executionResultUi: ExecutionResultUi = {
  recordPinHistory: (projection) => useExecutionStore.getState().recordPinHistory(projection),
  getPinHistory: (graphPath, output) => useExecutionStore
    .getState()
    .getGraph(graphPath)
    .pinHistories
    .get(pinHistoryCacheKey(graphPath, output)),
  clearRunOutput: (graphPath) => useExecutionStore.getState().clearRunOutput(graphPath),
};
