import { SelectionMode } from "@xyflow/react";

const multiSelectionKeys = ["Shift", "Control", "Meta"];
const panButtons = [1, 2];

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
