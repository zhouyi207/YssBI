import {
  SelectionMode,
  type Edge,
  type Node,
  type Viewport,
  type useStoreApi,
} from "@xyflow/react";

const multiSelectionKeys = ["Shift", "Control", "Meta"];
const panButtons = [1, 2];

export function synchronizeFlowViewport<NodeType extends Node, EdgeType extends Edge>(
  store: ReturnType<typeof useStoreApi<NodeType, EdgeType>>,
  next: Viewport,
) {
  const { panZoom, transform } = store.getState();
  // Restore D3 too: a cancelled gesture may have moved its private transform after cancellation.
  panZoom?.syncViewport(next);
  if (transform[0] !== next.x || transform[1] !== next.y || transform[2] !== next.zoom)
    store.setState({ transform: [next.x, next.y, next.zoom] });
}

/** Shared pointer and native-key policy for the node graph and mind-map canvases. */
export function flowCanvasInteractionProps(interactive: boolean) {
  return {
    nodesDraggable: interactive,
    elementsSelectable: interactive,
    deleteKeyCode: null,
    disableKeyboardA11y: true,
    selectionOnDrag: interactive,
    selectionMode: SelectionMode.Partial,
    selectionKeyCode: null,
    multiSelectionKeyCode: multiSelectionKeys,
    panOnDrag: interactive ? panButtons : false,
    panActivationKeyCode: interactive ? "Alt" : null,
    zoomOnScroll: interactive,
    zoomOnPinch: interactive,
    zoomOnDoubleClick: false,
    autoPanOnNodeDrag: false,
    autoPanOnConnect: false,
    autoPanOnSelection: false,
  };
}
