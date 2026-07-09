import { create } from 'zustand';
import type {
  ExecutionState,
  GraphExecutionState,
  ExecutionEvent,
  RecordedEvent,
  PinResultState,
} from '@/shared/types/ui';
import { flushLiveExecutionEventsNow } from './executionLiveFeed';
import {
  clearExecutionVisual,
  getExecutionVisual,
  resetExecutionVisual,
  snapshotToGraphPatch,
} from './executionVisualSession';
import { clearedRunArtifactsPatch } from './graphRunArtifacts';
import { normalizePinResultState, type PinResultWirePayload } from './normalizePinResult';
import { pinResultCacheKey } from './pinResultIndex';

const emptyGraphState = (): GraphExecutionState => ({
  status: "idle",
  nodeStates: new Map(),
  completedConnections: new Set(),
  flowingConnections: new Set(),
  recording: [],
  graphDirty: false,
  pinResults: new Map(),
});

function clearedVisualPatch(): Pick<
  GraphExecutionState,
  'status' | 'nodeStates' | 'completedConnections' | 'flowingConnections'
> {
  return {
    status: "idle",
    nodeStates: new Map(),
    completedConnections: new Set(),
    flowingConnections: new Set(),
  };
}

function stopPlaybackIfGraph(
  state: ExecutionState,
  graphPath: string,
): Pick<ExecutionState, 'isPlaying' | 'playbackGraphPath'> {
  const stop = state.playbackGraphPath === graphPath;
  return {
    isPlaying: stop ? false : state.isPlaying,
    playbackGraphPath: stop ? null : state.playbackGraphPath,
  };
}

interface ExecutionStore extends ExecutionState {
  getGraph: (graphPath: string) => GraphExecutionState;

  /** Mark graph as running; clears prior run artifacts and node visuals. */
  startExecution: (graphPath: string) => void;
  completeExecution: (graphPath: string) => void;
  failExecution: (graphPath: string) => void;
  /** User cancelled a live run; clear partial pin results and replay data. */
  interruptExecution: (graphPath: string) => void;
  /** User cleared last run without executing again. */
  clearGraphRunArtifacts: (graphPath: string) => void;
  /** Flush live/replay visual session into store (single React update). */
  commitExecutionVisual: (graphPath: string) => void;
  recordPinResult: (graphPath: string, result: PinResultWirePayload | PinResultState) => void;
  setRecording: (graphPath: string, recording: RecordedEvent[]) => void;
  setPlaying: (playing: boolean, graphPath?: string) => void;
  markGraphDirty: (graphPath: string) => void;
  clearPinResults: (graphPath: string, pinIds: string[]) => void;
  /** Drop all execution state when a graph is fully closed (no open tab). */
  releaseGraphExecutionState: (graphPath: string) => void;
  /** Clear node/connection visuals only; keep pin results and recording (replay start). */
  resetGraphVisuals: (graphPath: string) => void;
  /** Side-effect events only (pin results). Visual events use executionVisualSession. */
  applySideEffectEvent: (graphPath: string, event: ExecutionEvent) => void;
}

function updateGraph(
  state: ExecutionState,
  graphPath: string,
  patch: Partial<GraphExecutionState>,
): { graphs: Record<string, GraphExecutionState> } {
  const prev = state.graphs[graphPath] ?? emptyGraphState();
  return {
    graphs: {
      ...state.graphs,
      [graphPath]: { ...prev, ...patch },
    },
  };
}

function commitVisualSnapshot(
  graphPath: string,
  set: (fn: (state: ExecutionState) => Partial<ExecutionState> | ExecutionState) => void,
): void {
  const snap = getExecutionVisual();
  if (snap.graphPath !== graphPath) {
    clearExecutionVisual();
    return;
  }
  const patch = snapshotToGraphPatch(snap);
  clearExecutionVisual();
  set((state) => updateGraph(state, graphPath, patch));
}

export const useExecutionStore = create<ExecutionStore>((set, get) => ({
  graphs: {},
  playbackGraphPath: null,
  isPlaying: false,

  getGraph: (graphPath) => get().graphs[graphPath] ?? emptyGraphState(),

  startExecution: (graphPath) => {
    resetExecutionVisual(graphPath);
    set((state) => updateGraph(state, graphPath, {
      ...clearedVisualPatch(),
      ...clearedRunArtifactsPatch(),
      status: "running",
    }));
  },

  completeExecution: (graphPath) => set((state) => updateGraph(state, graphPath, {
    status: "completed",
  })),

  failExecution: (graphPath) => set((state) => updateGraph(state, graphPath, {
    status: "error",
  })),

  interruptExecution: (graphPath) => {
    clearExecutionVisual();
    set((state) => ({
      ...updateGraph(state, graphPath, {
        ...clearedVisualPatch(),
        ...clearedRunArtifactsPatch(),
      }),
      ...stopPlaybackIfGraph(state, graphPath),
    }));
  },

  clearGraphRunArtifacts: (graphPath) => {
    clearExecutionVisual();
    set((state) => ({
      ...updateGraph(state, graphPath, {
        ...clearedVisualPatch(),
        ...clearedRunArtifactsPatch(),
      }),
      ...stopPlaybackIfGraph(state, graphPath),
    }));
  },

  commitExecutionVisual: (graphPath) => {
    flushLiveExecutionEventsNow();
    commitVisualSnapshot(graphPath, set);
  },

  recordPinResult: (graphPath, result) => set((state) => {
    const g = state.graphs[graphPath] ?? emptyGraphState();
    const normalized = normalizePinResultState(graphPath, result);
    const next = new Map(g.pinResults);
    next.set(pinResultCacheKey(normalized.graphPath, normalized.pinId), normalized);
    return updateGraph(state, graphPath, { pinResults: next });
  }),

  setRecording: (graphPath, recording) => set((state) => updateGraph(state, graphPath, { recording })),

  setPlaying: (playing, graphPath) => set({
    isPlaying: playing,
    playbackGraphPath: playing ? (graphPath ?? get().playbackGraphPath) : null,
  }),

  markGraphDirty: (graphPath) => set((state) => {
    const g = state.graphs[graphPath];
    if (!g) return state;
    if (g.status === "idle" && !(state.isPlaying && state.playbackGraphPath === graphPath)) {
      return state;
    }
    clearExecutionVisual();
    return {
      ...updateGraph(state, graphPath, {
        ...clearedVisualPatch(),
        graphDirty: true,
      }),
      ...stopPlaybackIfGraph(state, graphPath),
    };
  }),

  clearPinResults: (resultGraphPath, pinIds) => set((state) => {
    if (pinIds.length === 0) return state;

    let changed = false;
    const graphs = { ...state.graphs };

    for (const [bucketPath, bucket] of Object.entries(graphs)) {
      if (bucket.pinResults.size === 0) continue;

      const next = new Map(bucket.pinResults);
      for (const pinId of pinIds) {
        if (next.delete(pinResultCacheKey(resultGraphPath, pinId))) {
          changed = true;
        }
      }

      if (next.size !== bucket.pinResults.size) {
        graphs[bucketPath] = { ...bucket, pinResults: next };
      }
    }

    return changed ? { ...state, graphs } : state;
  }),

  releaseGraphExecutionState: (graphPath) => set((state) => {
    if (!state.graphs[graphPath]) return state;
    const graphs = { ...state.graphs };
    delete graphs[graphPath];
    clearExecutionVisual();
    return {
      graphs,
      ...stopPlaybackIfGraph(state, graphPath),
    };
  }),

  resetGraphVisuals: (graphPath) => set((state) => {
    clearExecutionVisual();
    return {
      ...updateGraph(state, graphPath, clearedVisualPatch()),
      ...stopPlaybackIfGraph(state, graphPath),
    };
  }),

  applySideEffectEvent: (graphPath, event) => {
    if (event.event === 'pinResultReady') {
      get().recordPinResult(graphPath, event.data);
    }
  },
}));
