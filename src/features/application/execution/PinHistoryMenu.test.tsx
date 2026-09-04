// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ResultService } from "@/services/result/resultService";
import { useExecutionStore } from "@/features/core/execution/useExecutionStore";
import type { PinResultEntry } from "@/shared/types/domain/result";
import type { PortAddressDto } from "@/shared/types/dto/editorProjection";
import { openInspectableResult } from "./openInspectableResult";
import { PinHistoryMenu } from "./PinHistoryMenu";

vi.mock("@/services/result/resultService", () => ({
  ResultService: { getPinHistory: vi.fn() },
}));
vi.mock("./openInspectableResult", () => ({
  openInspectableResult: vi.fn().mockResolvedValue(true),
}));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const graphPath = "events/Main.yssbi-event";
const output: PortAddressDto = {
  kind: "declared",
  nodeId: "node-1",
  portKey: "result",
};

function entry(
  resultId: string,
  state: PinResultEntry["state"],
  createdAtMs: string,
): PinResultEntry {
  return {
    resultId,
    runId: `run-${resultId}`,
    activationId: `activation-${resultId}`,
    createdAtMs,
    usage: { kind: "produced" },
    state,
  };
}

describe("PinHistoryMenu", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    useExecutionStore.setState({ graphs: {}, playbackGraphPath: null, isPlaying: false });
    vi.mocked(ResultService.getPinHistory).mockResolvedValue([
      entry("17", { kind: "ready" }, "1000"),
      entry("18", { kind: "cancelled" }, "2000"),
    ]);
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    document
      .querySelectorAll('[data-slot="dropdown-menu-content"]')
      .forEach((node) => node.remove());
  });

  it("queries exact output history and shows latest first with metadata", async () => {
    act(() =>
      root.render(<PinHistoryMenu graphPath={graphPath} outputs={[output]} label="View" />),
    );

    await act(async () => {
      const trigger = container.querySelector<HTMLButtonElement>("button");
      trigger?.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, button: 0 }));
      trigger?.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, button: 0 }));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(ResultService.getPinHistory).toHaveBeenCalledWith(graphPath, output);
    const items = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')];
    expect(items.map((item) => item.textContent)).toEqual([
      expect.stringContaining("18"),
      expect.stringContaining("17"),
    ]);
    expect(items[0]?.textContent).toContain("cancelled");
    expect(items[0]?.textContent).toContain("run-18");
    expect(items[0]?.textContent).toContain("Latest");
  });

  it("opens the selected historical exact result ID", async () => {
    act(() =>
      root.render(<PinHistoryMenu graphPath={graphPath} outputs={[output]} label="View" />),
    );
    await act(async () => {
      const trigger = container.querySelector<HTMLButtonElement>("button");
      trigger?.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, button: 0 }));
      trigger?.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, button: 0 }));
      await Promise.resolve();
      await Promise.resolve();
    });

    const historical = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(
      (item) => item.textContent?.includes("17"),
    );
    await act(async () => {
      historical?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await Promise.resolve();
    });

    expect(openInspectableResult).toHaveBeenCalledWith(
      { kind: "result", resultId: "17" },
      expect.any(Function),
    );
    expect(
      useExecutionStore.getState().getGraph(graphPath).pinHistories.values().next().value,
    ).toMatchObject({ selectedResultId: "17" });
  });
});
