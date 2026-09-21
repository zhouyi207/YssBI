import { beforeEach, describe, expect, it, vi } from "vitest";
import { useEditorStore } from "@/features/core/editor";

const mocks = vi.hoisted(() => ({ reveal: vi.fn() }));
vi.mock("@/modules/workbench/public", () => ({
  workbenchLayoutRead: { isReady: true },
  revealWorkbenchView: mocks.reveal,
}));
import {
  revealDetails,
  setDetailContext,
  setPassiveDetailContext,
  setInspectionContext,
} from "./rightSidebarActions";

beforeEach(() => {
  vi.clearAllMocks();
  useEditorStore.setState({ detailFocus: null });
});

describe("right sidebar context actions", () => {
  it("keeps explicit node focus when passive graph hydration reports the same tab", () => {
    setInspectionContext("events/Main.yssbi-event", ["node-1"]);
    setPassiveDetailContext({ kind: "event", path: "events/Main.yssbi-event" });
    expect(useEditorStore.getState().detailFocus).toEqual({
      kind: "node",
      id: "node-1",
      graphPath: "events/Main.yssbi-event",
    });
  });

  it("publishes node context before awaiting Details reveal", async () => {
    let resolve!: () => void;
    mocks.reveal.mockImplementation(
      () =>
        new Promise<void>((done) => {
          resolve = done;
        }),
    );
    let settled = false;
    const revealing = revealDetails({
      kind: "node",
      id: "node-2",
      graphPath: "events/Main.yssbi-event",
    }).then(() => {
      settled = true;
    });
    expect(useEditorStore.getState().detailFocus).toEqual({
      kind: "node",
      id: "node-2",
      graphPath: "events/Main.yssbi-event",
    });
    expect(mocks.reveal).toHaveBeenCalledWith("details");
    await Promise.resolve();
    expect(settled).toBe(false);
    resolve();
    await revealing;
    expect(settled).toBe(true);
  });

  it("preserves non-node Details context when selection clears without opening panels", () => {
    setDetailContext({ kind: "data", id: "database-1" });
    setInspectionContext("events/Main.yssbi-event", []);
    expect(useEditorStore.getState().detailFocus).toEqual({ kind: "data", id: "database-1" });
    expect(mocks.reveal).not.toHaveBeenCalled();
  });
});
