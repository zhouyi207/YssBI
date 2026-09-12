import { expect, it, vi } from "vitest";
import type { ResultDescriptor } from "@/shared/types/domain/result";
import { resultReferenceFixture } from "@/tests/helpers/resultFixture";
import { createResultLeaseController } from "./resultLeases";

const descriptor: ResultDescriptor = {
  ...resultReferenceFixture("1"),
  provenance: {
    runId: "1",
    graphPath: "events/main.yssbi-event",
    nodeId: "node",
    output: null,
    createdAtMs: "1",
  },
  presentation: { kind: "inspector" },
  valueKind: "scalar",
  metadata: null,
  totalCount: 1,
  title: "Result",
};

function setup() {
  const port = {
    retain: vi.fn(async (_reference, leaseId: string, _handoff?: string) => ({
      leaseId,
      descriptor,
    })),
    release: vi.fn(async (_leaseId: string) => {}),
    reconcileLeases: vi.fn(async (_leases: readonly string[]) => {}),
  };
  const failure = vi.fn();
  return { port, failure, controller: createResultLeaseController(port, failure) };
}

it("releases the known request token after a lost retain response or failed window creation", async () => {
  const { controller, port } = setup();
  controller.bind(() => []);
  port.retain.mockRejectedValueOnce(new Error("lost response"));
  await expect(controller.acquire(descriptor)).rejects.toThrow("lost response");
  expect(port.release).toHaveBeenCalledWith(port.retain.mock.calls[0][1]);
  const held = await controller.acquire(descriptor, "plot-child");
  expect(port.retain).toHaveBeenLastCalledWith(descriptor, held.leaseId, "plot-child");
  await controller.finish(held.leaseId, false);
  await controller.whenIdle();
  expect(port.release).toHaveBeenLastCalledWith(held.leaseId);
  expect(port.reconcileLeases).toHaveBeenLastCalledWith([]);
});
