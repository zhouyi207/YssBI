// @vitest-environment happy-dom
import { act, useRef } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  refreshCatalog: vi.fn(),
  setDropHandler: vi.fn(),
}));

vi.mock("@/features/core/sidebarDrag", () => ({
  canvasDropHandlerStore: {
    setHandler: mocks.setDropHandler,
  },
}));

vi.mock("@/features/application/nodeCatalog/useLocalizedNodeCatalog", () => ({
  useLocalizedNodeCatalog: () => ({
    catalog: null,
    refresh: mocks.refreshCatalog,
  }),
}));

import { useCanvasDrop } from "./useCanvasDrop";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function PreviewCanvasDropProbe() {
  const canvasElementRef = useRef<HTMLDivElement>(null);
  useCanvasDrop({
    canvasElementRef,
    panelInstanceId: "editor-a",
    groupId: "group-a",
    graphPath: "events/Main.yssbi-event",
    variables: {},
    setContextMenu: vi.fn(),
    setPendingConnection: vi.fn(),
    createNode: vi.fn(async () => true),
    enabled: false,
  });
  return <div ref={canvasElementRef} />;
}

describe("useCanvasDrop preview registration", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("keeps the panel drop handler registered until a preview canvas unmounts", () => {
    act(() => root.render(<PreviewCanvasDropProbe />));

    expect(mocks.setDropHandler).toHaveBeenCalledWith("editor-a", expect.any(Function));

    act(() => root.render(null));
    expect(mocks.setDropHandler).toHaveBeenLastCalledWith("editor-a", null);
  });
});
