export interface ExecutionUiCapability {
  readonly setPlaying: (graphPath: string | null, playing: boolean) => void;
  readonly setRecording: (graphPath: string, recording: boolean) => void;
}
