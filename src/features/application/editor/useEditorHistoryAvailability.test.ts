// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useGraphEditingStore } from "@/features/core/graphEditing";
import { useEditorHistoryAvailability } from "./useEditorHistoryAvailability";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const graphPath = "events/Main.yssbi-event";
const activeEditor = vi.hoisted(() => ({
  activeResourceRef: "events/Main.yssbi-event" as string | null,
}));

vi.mock("./editorGroupContext", () => ({
  useActiveEditorGroup: () => ({ activeResourceRef: activeEditor.activeResourceRef }),
}));

const draftDocument = { nodes: {}, port_bindings: [], connections: {}, input_states: [] };
const version = { document: draftDocument, projection: {} as never };

describe("useEditorHistoryAvailability", () => {
  let host: HTMLDivElement;
  let root: Root;
  let current: ReturnType<typeof useEditorHistoryAvailability> | undefined;

  function Harness() {
    current = useEditorHistoryAvailability();
    return null;
  }

  beforeEach(() => {
    activeEditor.activeResourceRef = graphPath;
    useGraphEditingStore.setState({ sessions: {} });
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("derives undo/redo only from the active Rust graph projection and masks both while saving", () => {
    useGraphEditingStore.setState({
      sessions: {
        [graphPath]: {
          ...version,
          sessionId: 1,
          projectionGeneration: 0,
          semanticInputHash: "0".repeat(64),
          saveDirty: false,
          version: { sessionId: "00000000-0000-0000-0000-000000000090", revision: "0" },
          saving: false,
          canUndo: true,
          canRedo: true,
        },
      },
    });
    act(() => root.render(createElement(Harness)));
    expect(current).toEqual({
      activeResourceRef: graphPath,
      canUndo: true,
      canRedo: true,
      pending: false,
    });

    act(() => useGraphEditingStore.getState().beginSave(graphPath));
    expect(current).toEqual({
      activeResourceRef: graphPath,
      canUndo: false,
      canRedo: false,
      pending: true,
    });

    activeEditor.activeResourceRef = null;
    act(() => root.render(createElement(Harness)));
    expect(current).toEqual({
      activeResourceRef: null,
      canUndo: false,
      canRedo: false,
      pending: false,
    });
  });
});
