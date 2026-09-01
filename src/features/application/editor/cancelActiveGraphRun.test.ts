import { beforeEach, describe, expect, it, vi } from "vitest";
import { useExecutionStore } from "@/features/core/execution";
import { cancelActiveGraphRun } from "./cancelActiveGraphRun";

describe("cancelActiveGraphRun", () => {
  beforeEach(() => {
    useExecutionStore.setState({
      graphs: {},
      playbackGraphPath: null,
      isPlaying: false,
    });
  });

  it("forwards the projected opaque run ID to the cancellation service", async () => {
    const cancelGraphRun = vi.fn().mockResolvedValue(true);
    const graphPath = "events/Main.yssbi-event";
    useExecutionStore.getState().startExecution(graphPath);
    useExecutionStore.getState().setActiveRunId(graphPath, "9007199254740993");

    await expect(cancelActiveGraphRun(graphPath, { cancelGraphRun })).resolves.toBe(true);

    expect(cancelGraphRun).toHaveBeenCalledOnce();
    expect(cancelGraphRun).toHaveBeenCalledWith("9007199254740993");
  });

  it("does not invoke cancellation before runStarted supplies an ID", async () => {
    const cancelGraphRun = vi.fn().mockResolvedValue(true);
    const graphPath = "events/Main.yssbi-event";
    useExecutionStore.getState().startExecution(graphPath);

    await expect(cancelActiveGraphRun(graphPath, { cancelGraphRun })).resolves.toBe(false);

    expect(cancelGraphRun).not.toHaveBeenCalled();
  });
});
