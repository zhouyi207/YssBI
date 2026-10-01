import { useCallback, useEffect, useLayoutEffect, useMemo, useRef } from "react";
import { useStoreApi, type NodeChange, type NodeDimensionChange } from "@xyflow/react";
import type { EditorCanvasSession } from "@/features/application/editor";
import {
  createGraphFlowNodeViewProjector,
  type GraphFlowModel,
  type GraphFlowNode,
  type GraphFlowEdge,
} from "./graphFlowModel";

type Interaction = EditorCanvasSession["interaction"];
type GestureLease = NonNullable<ReturnType<Interaction["beginGesture"]>>;
type PositionPreview = {
  position: { x: number; y: number };
  dragging: boolean;
  owner: object;
};

interface GraphFlowNodesOptions {
  graphPath: string;
  model: GraphFlowModel;
  selectedNodeIds: readonly string[];
  interactive: boolean;
  interaction: Interaction;
  onCancel(): void;
}

/** Own the panel's node overlays and their publication to React Flow. */
export function useGraphFlowNodes({
  graphPath,
  model,
  selectedNodeIds,
  interactive,
  interaction,
  onCancel,
}: GraphFlowNodesOptions) {
  const flowStore = useStoreApi<GraphFlowNode, GraphFlowEdge>();
  const nodeViews = useMemo(createGraphFlowNodeViewProjector, []);
  const selected = useMemo(() => new Set(selectedNodeIds), [selectedNodeIds]);
  const inputs = useRef({
    model,
    selectedNodeIds: selected,
    interactive,
  });
  const positions = useRef<ReadonlyMap<string, PositionPreview>>(new Map());
  const drag = useRef<{ lease: GestureLease; owner: object } | null>(null);
  const mounted = useRef(true);
  const previewFrame = useRef<number | null>(null);
  const pendingPreview = useRef(false);

  const cancelFrame = useCallback(() => {
    if (previewFrame.current !== null) cancelAnimationFrame(previewFrame.current);
    previewFrame.current = null;
    pendingPreview.current = false;
  }, []);
  const publishNodes = useCallback(() => {
    pendingPreview.current = false;
    const nodes = nodeViews.project({ ...inputs.current, positions: positions.current });
    const state = flowStore.getState();
    // The renderer still owns node lookup and handle measurements through its adoption action.
    if (state.nodes !== nodes) state.setNodes(nodes);
  }, [flowStore, nodeViews]);
  const synchronizeNodes = useCallback(() => {
    cancelFrame();
    publishNodes();
  }, [cancelFrame, publishNodes]);
  const clearPositions = useCallback(
    (owner: object) => {
      if (!mounted.current) return;
      const current = positions.current;
      let next: Map<string, PositionPreview> | undefined;
      for (const [id, entry] of current) {
        if (entry.owner !== owner) continue;
        next ??= new Map(current);
        next.delete(id);
      }
      if (!next) return;
      positions.current = next;
      synchronizeNodes();
    },
    [synchronizeNodes],
  );

  const schedulePreview = useCallback(() => {
    if (previewFrame.current !== null) {
      pendingPreview.current = true;
      return;
    }
    const current = drag.current;
    if (!current) return;
    // Preserve immediate response at normal input rates; coalesce only additional samples.
    publishNodes();
    let scheduled: number;
    const flush = () => {
      if (previewFrame.current !== scheduled) return;
      previewFrame.current = null;
      if (!mounted.current || drag.current !== current) return;
      if (!current.lease.isCurrent() || !interaction.isInteractive()) {
        clearPositions(current.owner);
        return;
      }
      if (!pendingPreview.current) return;
      publishNodes();
      scheduled = requestAnimationFrame(flush);
      previewFrame.current = scheduled;
    };
    scheduled = requestAnimationFrame(flush);
    previewFrame.current = scheduled;
  }, [publishNodes, clearPositions, interaction.isInteractive]);

  // Low-frequency inputs and terminal events use the same immediate publication boundary.
  useLayoutEffect(() => {
    inputs.current = { model, selectedNodeIds: selected, interactive };
    synchronizeNodes();
  }, [model, selected, interactive, synchronizeNodes]);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      cancelFrame();
      drag.current?.lease.finish();
      drag.current = null;
    };
  }, [cancelFrame]);

  const startNodeDrag = useCallback(() => {
    if (drag.current?.lease.isCurrent()) return;
    cancelFrame();
    const owner = {};
    const lease = interaction.beginGesture("draggingNodes", () => {
      onCancel();
      clearPositions(owner);
    });
    drag.current = lease ? { lease, owner } : null;
  }, [interaction.beginGesture, onCancel, clearPositions, cancelFrame]);
  const stopNodeDrag = useCallback(() => {
    cancelFrame();
    const current = drag.current;
    drag.current = null;
    if (!current) return;
    const submitted: { nodeId: string; position: { x: number; y: number } }[] = [];
    for (const [nodeId, entry] of positions.current) {
      if (entry.owner === current.owner) submitted.push({ nodeId, position: entry.position });
    }
    const valid = current.lease.isCurrent();
    current.lease.finish();
    if (!valid || !submitted.length) {
      clearPositions(current.owner);
      return;
    }
    const settled = new Map(positions.current);
    for (const [id, entry] of settled) {
      if (entry.owner === current.owner) settled.set(id, { ...entry, dragging: false });
    }
    positions.current = settled;
    // The latest accepted sample, including an unpainted tail, is committed exactly once.
    synchronizeNodes();
    void interaction.mutations
      .submitNodePositions(graphPath, submitted)
      .then((outcome) => {
        if (outcome.status === "failed")
          interaction.mutations.reportMutationFailure({ graphPath, intent: "moveNodes" });
      })
      .catch(() => interaction.mutations.reportMutationFailure({ graphPath, intent: "moveNodes" }))
      .finally(() => clearPositions(current.owner));
  }, [graphPath, interaction.mutations, cancelFrame, clearPositions, synchronizeNodes]);
  const updateNodes = useCallback(
    (changes: NodeChange<GraphFlowNode>[]) => {
      const current =
        interaction.isInteractive() && drag.current?.lease.isCurrent() ? drag.current : null;
      const before = inputs.current;
      let measurements: NodeDimensionChange[] | undefined;
      let next: Map<string, PositionPreview> | undefined;
      for (const change of changes) {
        if (change.type === "dimensions") {
          (measurements ??= []).push(change);
        } else if (
          change.type === "position" &&
          current &&
          change.position &&
          before.model.draggableNodeIds.has(change.id) &&
          Number.isFinite(change.position.x) &&
          Number.isFinite(change.position.y)
        ) {
          const previous = next?.get(change.id) ?? positions.current.get(change.id);
          const dragging = change.dragging ?? true;
          if (
            previous?.owner === current.owner &&
            previous.position.x === change.position.x &&
            previous.position.y === change.position.y &&
            previous.dragging === dragging
          )
            continue;
          next ??= new Map(positions.current);
          next.set(change.id, { position: { ...change.position }, dragging, owner: current.owner });
        }
      }
      if (next) positions.current = next;
      if (measurements && nodeViews.updateMeasurements(measurements)) {
        synchronizeNodes();
      } else if (next) schedulePreview();
    },
    [interaction.isInteractive, nodeViews, synchronizeNodes, schedulePreview],
  );
  const isDragging = useCallback(() => drag.current?.lease.isCurrent() ?? false, []);
  const recoverCancelledDrag = useCallback(() => {
    if (!drag.current || drag.current.lease.isCurrent()) return;
    cancelFrame();
    drag.current.lease.finish();
    drag.current = null;
  }, [cancelFrame]);

  return { updateNodes, startNodeDrag, stopNodeDrag, isDragging, recoverCancelledDrag };
}
