import { describe, expect, it } from "vitest";
import { recordingHadError, firstNodeErrorMessage } from "./executionRecording";
import type { RecordedEvent } from "@/features/core/execution/executionTypes";

function entry(event: RecordedEvent["event"]): RecordedEvent {
  return { event, timestamp: 0 };
}

describe("executionRecording", () => {
  it("recordingHadError prefers executionComplete over nodeError", () => {
    const recording: RecordedEvent[] = [
      entry({ event: "nodeError", data: { nodeId: "n1", error: "boom" } }),
      entry({ event: "executionComplete", data: { hasError: false } }),
    ];
    expect(recordingHadError(recording)).toBe(false);
  });

  it("recordingHadError falls back to nodeError without executionComplete", () => {
    const recording: RecordedEvent[] = [
      entry({ event: "nodeError", data: { nodeId: "n1", error: "boom" } }),
    ];
    expect(recordingHadError(recording)).toBe(true);
    expect(firstNodeErrorMessage(recording)).toBe("boom");
  });
});
