import { create } from 'zustand';
import type {
  ExecutionState,
  GraphExecutionState,
  RecordedEvent,
  PinHistoryProjection,
} from '@/shared/types/ui';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import type { RunOutputChannelEvent } from '@/shared/types/dto/runEvent';
import { flushLiveExecutionEventsNow } from './executionLiveFeed';
import {
  clearExecutionVisual,
  getExecutionVisual,
  resetExecutionVisual,
  snapshotToGraphPatch,
} from './executionVisualSession';
import { clearedRunProjectionsPatch } from './graphRunArtifacts';
import { pinHistoryCacheKey, pinPreviewCacheKey } from './pinResultIndex';
import { appendRunOutput, emptyRunOutputProjection } from './runOutputProjection';

const emptyGraphState = (): GraphExecutionState => ({
  status: "idle",
  runId: null,
  nodeStates: new Map(),
  completedConnections: new Set(),
  flowingConnections: new Set(),
  recording: [],
  graphDirty: false,
  runOutput: emptyRunOutputProjection(),
  pinHistories: new Map(),
  pinPreviews: new Map(),
});

function clearedVisualPatch(): Pick<
  GraphExecutionState,
  'status' | 'runId' | 'nodeStates' | 'completedConnections' | 'flowingConnections'
> {
  return {
    status: "idle",
    runId: null,
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

export interface PinPreviewLease {
  readonly generation: number;
  isCurrent: () => boolean;
  complete: (resultId: string) => boolean;
  fail: (error: string) => boolean;
  revoke: () => void;
}

type LeaseRecord = {
  graphPath: string;
  port: PortAddressDto;
  revoked: boolean;
  lease: PinPreviewLease;
};

const activePreviewLeases = new Map<string, LeaseRecord>();

function revokeGraphPreviewLeases(graphPath: string): void {
  for (const [key, record] of activePreviewLeases) {
    if (record.graphPath !== graphPath) continue;
    record.revoked = true;
    activePreviewLeases.delete(key);
  }
}

export function revokeAllPinPreviewLeases(): void {
  for (const record of activePreviewLeases.values()) record.revoked = true;
  activePreviewLeases.clear();
}

interface ExecutionStore extends ExecutionState {
  getGraph: (graphPath: string) => GraphExecutionState;

  /** Mark graph as running; clears prior run artifacts and node visuals. */
  startExecution: (graphPath: string) => void;
  setActiveRunId: (graphPath: string, runId: string) => void;
  completeExecution: (graphPath: string) => void;
  failExecution: (graphPath: string) => void;
  /** User cancelled a live run; clear partial pin results and replay data. */
  interruptExecution: (graphPath: string) => void;
  /** User cleared last run without executing again. */
  clearGraphRunProjections: (graphPath: string) => void;
  /** Flush live/replay visual session into store (single React update). */
  commitExecutionVisual: (graphPath: string) => void;
  recordPinHistory: (projection: PinHistoryProjection) => void;
  recordRunOutput: (graphPath: string, event: RunOutputChannelEvent) => void;
  clearRunOutput: (graphPath: string) => void;
  beginPinPreview: (
    graphPath: string,
    port: PortAddressDto,
    generation: number,
  ) => PinPreviewLease;
  completePinPreview: (
    graphPath: string,
    port: PortAddressDto,
    generation: number,
    resultId: string,
  ) => boolean;
  failPinPreview: (
    graphPath: string,
    port: PortAddressDto,
    generation: number,
    error: string,
  ) => boolean;
  removePinPreview: (
    graphPath: string,
    port: PortAddressDto,
    generation: number,
  ) => boolean;
  setRecording: (graphPath: string, recording: RecordedEvent[]) => void;
  setPlaying: (playing: boolean, graphPath?: string) => void;
  markGraphDirty: (graphPath: string) => void;

  /** Drop all execution state when a graph is fully closed (no open tab). */
  releaseGraphExecutionState: (graphPath: string) => void;
  /** Clear node/connection visuals only; keep pin results and recording (replay start). */
  resetGraphVisuals: (graphPath: string) => void;

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
      ...clearedRunProjectionsPatch(),
      runOutput: emptyRunOutputProjection(),
      status: "running",
    }));
  },

  setActiveRunId: (graphPath, runId) => set((state) => {
    const graph = state.graphs[graphPath];
    if (graph?.status !== 'running') return state;
    return updateGraph(state, graphPath, { runId });
  }),

  completeExecution: (graphPath) => set((state) => updateGraph(state, graphPath, {
    status: "completed",
    runId: null,
  })),

  failExecution: (graphPath) => set((state) => updateGraph(state, graphPath, {
    status: "error",
    runId: null,
  })),

  interruptExecution: (graphPath) => {
    clearExecutionVisual();
    set((state) => ({
      ...updateGraph(state, graphPath, {
        ...clearedVisualPatch(),
        ...clearedRunProjectionsPatch(),
      }),
      ...stopPlaybackIfGraph(state, graphPath),
    }));
  },

  clearGraphRunProjections: (graphPath) => {
    clearExecutionVisual();
    set((state) => ({
      ...updateGraph(state, graphPath, {
        ...clearedVisualPatch(),
        ...clearedRunProjectionsPatch(),
        runOutput: emptyRunOutputProjection(),
      }),
      ...stopPlaybackIfGraph(state, graphPath),
    }));
  },

  commitExecutionVisual: (graphPath) => {
    flushLiveExecutionEventsNow();
    commitVisualSnapshot(graphPath, set);
  },


  recordPinHistory: (projection) => set((state) => {
    const graph = state.graphs[projection.graphPath] ?? emptyGraphState();
    const pinHistories = new Map(graph.pinHistories);
    pinHistories.set(
      pinHistoryCacheKey(projection.graphPath, projection.output),
      projection,
    );
    return updateGraph(state, projection.graphPath, { pinHistories });
  }),

  recordRunOutput: (graphPath, event) => set((state) => {
    const graph = state.graphs[graphPath];
    if (!graph || graph.status !== 'running') return state;
    if (graph.runId !== null && graph.runId !== event.runId) return state;
    const runOutput = appendRunOutput(graph.runOutput, event);
    if (runOutput === graph.runOutput) return state;
    return updateGraph(state, graphPath, { runOutput });
  }),

  clearRunOutput: (graphPath) => set((state) => {
    if (!state.graphs[graphPath]) return state;
    return updateGraph(state, graphPath, { runOutput: emptyRunOutputProjection() });
  }),

  beginPinPreview: (graphPath, port, generation) => {
    const key = pinPreviewCacheKey(graphPath, port);
    const previous = activePreviewLeases.get(key);
    if (previous) previous.revoked = true;

    let record!: LeaseRecord;
    const lease: PinPreviewLease = {
      generation,
      isCurrent: () => !record.revoked && activePreviewLeases.get(key) === record,
      complete: (resultId) => lease.isCurrent()
        && useExecutionStore.getState().completePinPreview(graphPath, port, generation, resultId),
      fail: (error) => lease.isCurrent()
        && useExecutionStore.getState().failPinPreview(graphPath, port, generation, error),
      revoke: () => {
        record.revoked = true;
        if (activePreviewLeases.get(key) === record) activePreviewLeases.delete(key);
      },
    };
    record = { graphPath, port, revoked: false, lease };
    activePreviewLeases.set(key, record);

    set((state) => {
      const graph = state.graphs[graphPath] ?? emptyGraphState();
      const pinPreviews = new Map(graph.pinPreviews);
      pinPreviews.set(key, {
        graphPath,
        port,
        generation,
        status: 'pending',
        resultId: null,
        error: null,
      });
      return updateGraph(state, graphPath, { pinPreviews });
    });
    return lease;
  },

  completePinPreview: (graphPath, port, generation, resultId) => {
    let accepted = false;
    set((state) => {
      const graph = state.graphs[graphPath];
      if (!graph) return state;
      const key = pinPreviewCacheKey(graphPath, port);
      const preview = graph.pinPreviews.get(key);
      if (!preview || preview.generation !== generation) return state;
      const pinPreviews = new Map(graph.pinPreviews);
      pinPreviews.set(key, {
        ...preview,
        status: 'ready',
        resultId,
        error: null,
      });
      accepted = true;
      return updateGraph(state, graphPath, { pinPreviews });
    });
    return accepted;
  },

  failPinPreview: (graphPath, port, generation, error) => {
    let accepted = false;
    set((state) => {
      const graph = state.graphs[graphPath];
      if (!graph) return state;
      const key = pinPreviewCacheKey(graphPath, port);
      const preview = graph.pinPreviews.get(key);
      if (!preview || preview.generation !== generation) return state;
      const pinPreviews = new Map(graph.pinPreviews);
      pinPreviews.set(key, {
        ...preview,
        status: 'error',
        resultId: null,
        error,
      });
      accepted = true;
      return updateGraph(state, graphPath, { pinPreviews });
    });
    return accepted;
  },

  removePinPreview: (graphPath, port, generation) => {
    let removed = false;
    set((state) => {
      const graph = state.graphs[graphPath];
      if (!graph) return state;
      const key = pinPreviewCacheKey(graphPath, port);
      const preview = graph.pinPreviews.get(key);
      if (!preview || preview.generation !== generation) return state;
      const pinPreviews = new Map(graph.pinPreviews);
      pinPreviews.delete(key);
      removed = true;
      return updateGraph(state, graphPath, { pinPreviews });
    });
    return removed;
  },

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


  releaseGraphExecutionState: (graphPath) => {
    revokeGraphPreviewLeases(graphPath);
    set((state) => {
      if (!state.graphs[graphPath]) return state;
      const graphs = { ...state.graphs };
      delete graphs[graphPath];
      clearExecutionVisual();
      return {
        graphs,
        ...stopPlaybackIfGraph(state, graphPath),
      };
    });
  },

  resetGraphVisuals: (graphPath) => set((state) => {
    clearExecutionVisual();
    return {
      ...updateGraph(state, graphPath, clearedVisualPatch()),
      ...stopPlaybackIfGraph(state, graphPath),
    };
  }),

}));
