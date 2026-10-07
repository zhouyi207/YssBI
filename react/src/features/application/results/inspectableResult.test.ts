import { resultSessionFixture, resultReferenceFixture } from "@/tests/helpers/resultFixture";
import { describe, expect, it, vi } from "vitest";
import type { PortAddressDto } from "@/shared/types/dto/editorProjection";
import type { ResultDescriptor } from "./types";
import {
  outputPinRef,
  resolveInspectableResultRef,
  resultRef,
  type InspectableResultQueryDependencies,
} from "./inspectableResult";

const graphPath = "events/Main.yssbi-event";
const output: PortAddressDto = {
  kind: "instance",
  nodeId: "node-1",
  templateKey: "values",
  instanceId: "instance-2",
};

function createFixture() {
  return {
    coordinator: {
      loadPinResult: vi
        .fn<InspectableResultQueryDependencies["coordinator"]["loadPinResult"]>()
        .mockResolvedValue({ status: "published" }),
    },
    read: {
      getPinResult: vi
        .fn<InspectableResultQueryDependencies["read"]["getPinResult"]>()
        .mockReturnValue(null),
    },
  };
}

describe("resolveInspectableResultRef", () => {
  it("keeps an exact result reference without querying the current output", async () => {
    const fixture = createFixture();
    await expect(
      resolveInspectableResultRef(resultRef(resultReferenceFixture("17")), fixture),
    ).resolves.toEqual(resultReferenceFixture("17"));
    expect(fixture.coordinator.loadPinResult).not.toHaveBeenCalled();
  });

  it("resolves the current output reference and returns null when no result is published", async () => {
    const fixture = createFixture();
    const descriptor: ResultDescriptor = {
      resultId: "18",
      executionSessionId: resultSessionFixture,
      provenance: {
        runId: "2",
        createdAtMs: "1000",
        graphPath,
        nodeId: output.nodeId,
        output: { graphPath, port: output },
      },
      presentation: { kind: "inspector" },
      valueKind: "scalar",
      metadata: null,
      totalCount: 1,
      title: "Result",
    };
    fixture.read.getPinResult.mockReturnValueOnce(descriptor).mockReturnValueOnce(null);
    const ref = outputPinRef(graphPath, output);
    await expect(resolveInspectableResultRef(ref, fixture)).resolves.toEqual(
      resultReferenceFixture("18"),
    );
    await expect(resolveInspectableResultRef(ref, fixture)).resolves.toBeNull();
    expect(fixture.coordinator.loadPinResult).toHaveBeenNthCalledWith(1, { graphPath, output });
    expect(fixture.coordinator.loadPinResult).toHaveBeenNthCalledWith(2, { graphPath, output });
  });
});
