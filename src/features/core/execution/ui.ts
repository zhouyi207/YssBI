import type { RecordedEvent } from "@/features/core/execution/executionTypes";

export interface ExecutionUi {
  readonly setRecording: (graphPath: string, recording: readonly RecordedEvent[]) => void;
  readonly setPlaying: (playing: boolean, graphPath?: string) => void;
  readonly resetVisuals: (graphPath: string) => void;
}

import { useExecutionStore } from "./useExecutionStore";

export const executionUi: ExecutionUi = {
  setRecording: (graphPath, recording) =>
    useExecutionStore.getState().setRecording(graphPath, [...recording]),
  setPlaying: (playing, graphPath) => useExecutionStore.getState().setPlaying(playing, graphPath),
  resetVisuals: (graphPath) => useExecutionStore.getState().resetGraphVisuals(graphPath),
};

export interface ExecutionResultUi {
  readonly clearRunOutput: (graphPath: string) => void;
}

export const executionResultUi: ExecutionResultUi = {
  clearRunOutput: (graphPath) => useExecutionStore.getState().clearRunOutput(graphPath),
};
