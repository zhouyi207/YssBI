import { create } from 'zustand';
import type { ExecutionState, GraphExecutionState, ExecutionEvent, RecordedEvent, PinResultState } from '@/shared/types/ui';
import { flushLiveExecutionEventsNow } from './executionLiveFeed';
import { clearExecutionVisual, getExecutionVisual, snapshotToGraphPatch } from './executionVisualSession';

const emptyGraphState = (): GraphExecutionState => ({
  status: "idle",
  nodeStates: new Map(),
  completedConnections: new Set(),
  recording: [],
  graphDirty: false,
  pinResults: new Map(),
});

/** Clears committed node/connection visuals while preserving pin results & recording. */
function clearedVisualPatch(): Pick<
  GraphExecutionState,
  'status' | 'nodeStates' | 'completedConnections'
> {
  return {
    status: "idle",
    nodeStates: new Map(),
    completedConnections: new Set(),
  };
}

interface ExecutionStore extends ExecutionState {
  getGraph: (graphId: string) => GraphExecutionState;

  /** Mark graph as running; node visuals go through executionVisualSession until commit. */
  startExecution: (graphId: string) => void;
  completeExecution: (graphId: string) => void;
  failExecution: (graphId: string) => void;
  /** User cancelled a live run; keep partial visuals, return to idle. */
  interruptExecution: (graphId: string) => void;
  /** Flush live/replay visual session into store (single React update). */
  commitExecutionVisual: (graphId: string) => void;
  recordPinResult: (graphId: string, result: PinResultState) => void;
  setRecording: (graphId: string, recording: RecordedEvent[]) => void;
  setPlaying: (playing: boolean, graphId?: string) => void;
  markGraphDirty: (graphId: string) => void;
  clearPinResults: (graphId: string, pinIds: string[]) => void;
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

export const useExecutionStore = create<ExecutionStore>((set, get) => ({
  graphs: {},
  playbackGraphId: null,
  isPlaying: false,

  getGraph: (graphId) => get().graphs[graphId] ?? emptyGraphState(),

  startExecution: (graphId) => set((state) => updateGraph(state, graphId, {
    ...clearedVisualPatch(),
    status: "running",
    graphDirty: false,
    pinResults: new Map(),
  })),

  completeExecution: (graphId) => set((state) => updateGraph(state, graphId, {
    status: "completed",
  })),

  failExecution: (graphId) => set((state) => updateGraph(state, graphId, {
    status: "error",
  })),

  interruptExecution: (graphId) => set((state) => updateGraph(state, graphId, {
    status: "idle",
  })),

  commitExecutionVisual: (graphId) => {
    flushLiveExecutionEventsNow();
    const snap = getExecutionVisual();
    if (snap.graphId !== graphId) {
      clearExecutionVisual();
      return;
    }
    const patch = snapshotToGraphPatch(snap);
    clearExecutionVisual();
    set((state) => updateGraph(state, graphId, patch));
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
    const stop = state.playbackGraphId === graphId;
    clearExecutionVisual();
    return {
      ...updateGraph(state, graphId, {
        ...clearedVisualPatch(),
        graphDirty: true,
        recording: [],
      }),
      isPlaying: stop ? false : state.isPlaying,
      playbackGraphId: stop ? null : state.playbackGraphId,
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
    const stop = state.playbackGraphId === graphId;
    clearExecutionVisual();
    return {
      ...updateGraph(state, graphId, clearedVisualPatch()),
      isPlaying: stop ? false : state.isPlaying,
      playbackGraphId: stop ? null : state.playbackGraphId,
    };
  }),

  applySideEffectEvent: (graphId, event) => {
    if (event.event === 'pinResultReady') {
      get().recordPinResult(graphId, event.data);
    }
  },
}));
