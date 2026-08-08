import { create } from 'zustand';
import type {
  ExecutionState,
  GraphExecutionState,
  ExecutionEvent,
  RecordedEvent,
  PinResultState,
} from '@/shared/types/ui';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import { flushLiveExecutionEventsNow } from './executionLiveFeed';
import {
  clearExecutionVisual,
  getExecutionVisual,
  resetExecutionVisual,
  snapshotToGraphPatch,
} from './executionVisualSession';
import { clearedRunArtifactsPatch } from './graphRunArtifacts';
import { normalizePinResultState, type PinResultWirePayload } from './normalizePinResult';
import { pinPreviewCacheKey, pinResultCacheKey } from './pinResultIndex';

const emptyGraphState = (): GraphExecutionState => ({
  status: "idle",
  runId: null,
  nodeStates: new Map(),
  completedConnections: new Set(),
  flowingConnections: new Set(),
  recording: [],
  graphDirty: false,
  pinResults: new Map(),
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
  complete: (sourceId: string) => boolean;
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
  clearGraphRunArtifacts: (graphPath: string) => void;
  /** Flush live/replay visual session into store (single React update). */
  commitExecutionVisual: (graphPath: string) => void;
  recordPinResult: (graphPath: string, result: PinResultWirePayload | PinResultState) => void;
  beginPinPreview: (
    graphPath: string,
    port: PortAddressDto,
    generation: number,
  ) => PinPreviewLease;
  completePinPreview: (
    graphPath: string,
    port: PortAddressDto,
    generation: number,
    sourceId: string,
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

  beginPinPreview: (graphPath, port, generation) => {
    const key = pinPreviewCacheKey(graphPath, port);
    const previous = activePreviewLeases.get(key);
    if (previous) previous.revoked = true;

    let record!: LeaseRecord;
    const lease: PinPreviewLease = {
      generation,
      isCurrent: () => !record.revoked && activePreviewLeases.get(key) === record,
      complete: (sourceId) => lease.isCurrent()
        && useExecutionStore.getState().completePinPreview(graphPath, port, generation, sourceId),
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
        sourceId: null,
        error: null,
      });
      return updateGraph(state, graphPath, { pinPreviews });
    });
    return lease;
  },

  completePinPreview: (graphPath, port, generation, sourceId) => {
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
        sourceId,
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
        sourceId: null,
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

  applySideEffectEvent: (graphPath, event) => {
    if (event.event === 'pinResultReady') {
      get().recordPinResult(graphPath, event.data);
    }
  },
}));
