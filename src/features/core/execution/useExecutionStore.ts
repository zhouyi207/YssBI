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
  graphId: string,
): Pick<ExecutionState, 'isPlaying' | 'playbackGraphId'> {
  const stop = state.playbackGraphId === graphId;
  return {
    isPlaying: stop ? false : state.isPlaying,
    playbackGraphId: stop ? null : state.playbackGraphId,
  };
}

interface ExecutionStore extends ExecutionState {
  getGraph: (graphId: string) => GraphExecutionState;

  /** Mark graph as running; clears prior run artifacts and node visuals. */
  startExecution: (graphId: string) => void;
  completeExecution: (graphId: string) => void;
  failExecution: (graphId: string) => void;
  /** User cancelled a live run; clear partial pin results and replay data. */
  interruptExecution: (graphId: string) => void;
  /** User cleared last run without executing again. */
  clearGraphRunArtifacts: (graphId: string) => void;
  /** Flush live/replay visual session into store (single React update). */
  commitExecutionVisual: (graphId: string) => void;
  recordPinResult: (graphId: string, result: PinResultState) => void;
  setRecording: (graphId: string, recording: RecordedEvent[]) => void;
  setPlaying: (playing: boolean, graphId?: string) => void;
  markGraphDirty: (graphId: string) => void;
  clearPinResults: (graphId: string, pinIds: string[]) => void;
  /** Clear node/connection visuals only; keep pin results and recording (replay start). */
  resetGraphVisuals: (graphId: string) => void;
  /** Side-effect events only (pin results). Visual events use executionVisualSession. */
  applySideEffectEvent: (graphId: string, event: ExecutionEvent) => void;
}

function updateGraph(
  state: ExecutionState,
  graphId: string,
  patch: Partial<GraphExecutionState>,
): { graphs: Record<string, GraphExecutionState> } {
  const prev = state.graphs[graphId] ?? emptyGraphState();
  return {
    graphs: {
      ...state.graphs,
      [graphId]: { ...prev, ...patch },
    },
  };
}

function commitVisualSnapshot(
  graphId: string,
  set: (fn: (state: ExecutionState) => Partial<ExecutionState> | ExecutionState) => void,
): void {
  const snap = getExecutionVisual();
  if (snap.graphId !== graphId) {
    clearExecutionVisual();
    return;
  }
  const patch = snapshotToGraphPatch(snap);
  clearExecutionVisual();
  set((state) => updateGraph(state, graphId, patch));
}

export const useExecutionStore = create<ExecutionStore>((set, get) => ({
  graphs: {},
  playbackGraphId: null,
  isPlaying: false,

  getGraph: (graphId) => get().graphs[graphId] ?? emptyGraphState(),

  startExecution: (graphId) => {
    resetExecutionVisual(graphId);
    set((state) => updateGraph(state, graphId, {
      ...clearedVisualPatch(),
      ...clearedRunArtifactsPatch(),
      status: "running",
    }));
  },

  completeExecution: (graphId) => set((state) => updateGraph(state, graphId, {
    status: "completed",
  })),

  failExecution: (graphId) => set((state) => updateGraph(state, graphId, {
    status: "error",
  })),

  interruptExecution: (graphId) => {
    clearExecutionVisual();
    set((state) => ({
      ...updateGraph(state, graphId, {
        ...clearedVisualPatch(),
        ...clearedRunArtifactsPatch(),
      }),
      ...stopPlaybackIfGraph(state, graphId),
    }));
  },

  clearGraphRunArtifacts: (graphId) => {
    clearExecutionVisual();
    set((state) => ({
      ...updateGraph(state, graphId, {
        ...clearedVisualPatch(),
        ...clearedRunArtifactsPatch(),
      }),
      ...stopPlaybackIfGraph(state, graphId),
    }));
  },

  commitExecutionVisual: (graphId) => {
    flushLiveExecutionEventsNow();
    commitVisualSnapshot(graphId, set);
  },

  recordPinResult: (graphId, result) => set((state) => {
    const g = state.graphs[graphId] ?? emptyGraphState();
    const next = new Map(g.pinResults);
    next.set(result.pinId, result);
    return updateGraph(state, graphId, { pinResults: next });
  }),

  setRecording: (graphId, recording) => set((state) => updateGraph(state, graphId, { recording })),

  setPlaying: (playing, graphId) => set({
    isPlaying: playing,
    playbackGraphId: playing ? (graphId ?? get().playbackGraphId) : null,
  }),

  markGraphDirty: (graphId) => set((state) => {
    const g = state.graphs[graphId];
    if (!g || (g.status === "idle" && !(state.isPlaying && state.playbackGraphId === graphId))) return state;
    clearExecutionVisual();
    return {
      ...updateGraph(state, graphId, {
        ...clearedVisualPatch(),
        ...clearedRunArtifactsPatch(true),
      }),
      ...stopPlaybackIfGraph(state, graphId),
    };
  }),

  clearPinResults: (graphId, pinIds) => set((state) => {
    if (pinIds.length === 0) return state;
    const g = state.graphs[graphId];
    if (!g || g.pinResults.size === 0) return state;
    const next = new Map(g.pinResults);
    for (const pinId of pinIds) {
      next.delete(pinId);
    }
    return updateGraph(state, graphId, { pinResults: next });
  }),

  resetGraphVisuals: (graphId) => set((state) => {
    clearExecutionVisual();
    return {
      ...updateGraph(state, graphId, clearedVisualPatch()),
      ...stopPlaybackIfGraph(state, graphId),
    };
  }),

  applySideEffectEvent: (graphId, event) => {
    if (event.event === 'pinResultReady') {
      get().recordPinResult(graphId, event.data);
    }
  },
}));
