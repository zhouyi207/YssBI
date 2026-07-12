import { create } from 'zustand';
import { GraphPath, NodeId, PinId } from '@/shared/types';

interface Camera {
  zoom: number;
  offset: { x: number; y: number };
}

interface GraphRuntimeStore {
  activeGraphPath: GraphPath | null;

  selection: {
    nodes: Set<NodeId>;
    pins: Set<PinId>;
  };

  hoverPin: PinId | null;
  previewConnection?: {
    from: PinId;
    toPosition: { x: number; y: number };
  };

  camera: Camera;

  // ==========================
  // Actions
  // ==========================
  setActiveGraph(id: GraphPath | null): void;
  selectNode(id: NodeId, append?: boolean): void;
  clearSelection(): void;

  setCamera(patch: Partial<Camera>): void;

  setHoverPin(pinId: PinId | null): void;
  setPreviewConnection(from: PinId, toPosition: { x: number; y: number }): void;
  clearPreviewConnection(): void;
}

export const useGraphRuntimeStore = create<GraphRuntimeStore>((set) => ({
  activeGraphPath: null,

  selection: {
    nodes: new Set<NodeId>(),
    pins: new Set<PinId>(),
  },

  hoverPin: null,
  previewConnection: undefined,

  camera: {
    zoom: 1,
    offset: { x: 0, y: 0 },
  },

  // ==========================
  // Active Graph
  // ==========================
  setActiveGraph: (graphPath) => {
    set({
      activeGraphPath: graphPath,
      selection: {
        nodes: new Set(),
        pins: new Set(),
      },
      hoverPin: null,
      previewConnection: undefined,
      camera: { zoom: 1, offset: { x: 0, y: 0 } },
    });
  },

  // ==========================
  // Selection
  // ==========================
  selectNode: (id, append = false) => {
    set((state) => {
      const nodes: Set<NodeId> = append
        ? new Set(state.selection.nodes)
        : new Set<NodeId>();
      nodes.add(id);
      return { selection: { ...state.selection, nodes } };
    });
  },

  clearSelection: () =>
    set({
      selection: { nodes: new Set(), pins: new Set() },
    }),

  // ==========================
  // Camera
  // ==========================
  setCamera: (patch) =>
    set((state) => ({
      camera: { ...state.camera, ...patch },
    })),

  // ==========================
  // Hover / Preview
  // ==========================
  setHoverPin: (pinId) => set({ hoverPin: pinId }),
  setPreviewConnection: (from, toPosition) =>
    set({ previewConnection: { from, toPosition } }),
  clearPreviewConnection: () => set({ previewConnection: undefined }),
}));
