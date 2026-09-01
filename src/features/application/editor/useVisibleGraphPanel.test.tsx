// @vitest-environment happy-dom

import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useVisibleGraphPanel } from "./useVisibleGraphPanel";

const mocks = vi.hoisted(() => ({
  synchronizeVisibleGraphPanel: vi.fn(async () => true),
}));

vi.mock("./synchronizeVisibleGraphPanel", () => ({
  synchronizeVisibleGraphPanel: mocks.synchronizeVisibleGraphPanel,
}));

describe("useVisibleGraphPanel", () => {
  let host: HTMLDivElement;
  let root: Root;
  let visible = false;
  const scope = { groupId: "group-1", graphPath: "events/Main.yssbi-event" };

  function Harness() {
    useVisibleGraphPanel(visible, scope);
    return null;
  }

  beforeEach(() => {
    vi.clearAllMocks();
    visible = false;
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("waits for explicit panel visibility before synchronizing", async () => {
    await act(async () => {
      root.render(createElement(Harness));
      await Promise.resolve();
    });
    expect(mocks.synchronizeVisibleGraphPanel).not.toHaveBeenCalled();

    await act(async () => {
      visible = true;
      root.render(createElement(Harness));
      await Promise.resolve();
    });
    expect(mocks.synchronizeVisibleGraphPanel).toHaveBeenCalledWith(scope);
  });
});
