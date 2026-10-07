import { beforeEach, describe, expect, it, vi } from "vitest";
import { useExecutionStore } from "@/features/core/execution";
import { cancelActiveGraphRun } from "./cancelActiveGraphRun";

describe("cancelActiveGraphRun", () => {
  beforeEach(() => {
    useExecutionStore.setState({
      graphs: {},
    });
  });

  it("forwards the projected execution session and opaque run ID without output bindings", async () => {
    const cancelGraphRun = vi.fn().mockResolvedValue(true);
    const graphPath = "events/Main.yssbi-event";
    useExecutionStore.getState().submitExecution(graphPath);
    useExecutionStore.getState().applyRunEvent({
      resultRevision: "1",
      run: {
        graphPath,
        runId: "9007199254740993",
        executionSessionId: "session",
        semanticInputHash: "0".repeat(64),
      },
      kind: { type: "runStarted", outputs: [] },
    });

    await expect(cancelActiveGraphRun(graphPath, { cancelGraphRun })).resolves.toBe(true);

    expect(cancelGraphRun).toHaveBeenCalledOnce();
    expect(cancelGraphRun).toHaveBeenCalledWith("session", "9007199254740993");
  });

  it("does not invoke cancellation before runStarted supplies an ID", async () => {
    const cancelGraphRun = vi.fn().mockResolvedValue(true);
    const graphPath = "events/Main.yssbi-event";
    useExecutionStore.getState().submitExecution(graphPath);

    await expect(cancelActiveGraphRun(graphPath, { cancelGraphRun })).resolves.toBe(false);

    expect(cancelGraphRun).not.toHaveBeenCalled();
  });
});
