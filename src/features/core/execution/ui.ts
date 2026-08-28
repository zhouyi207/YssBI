import type { RecordedEvent } from '@/shared/types/ui/execution';

export interface ExecutionUi {
  readonly setRecording: (graphPath: string, recording: readonly RecordedEvent[]) => void;
  readonly setPlaying: (playing: boolean, graphPath?: string) => void;
  readonly resetVisuals: (graphPath: string) => void;
}
