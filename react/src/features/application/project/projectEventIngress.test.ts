import { describe, expect, it, vi } from "vitest";
import type { ProjectEvent } from "@/services/project/projectEventParser";
import type { ProjectEventStreamItem } from "@/services/project/projectEventStream";
import { createProjectEventIngress } from "./projectEventIngress";
import type { ProjectEventConsumptionOutcome, ProjectEventConsumer } from "./projectEventConsumer";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

async function flushQueue(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
}

function event(operationId: string): ProjectEvent {
  return {
    type: "ResourceMutationCommitted",
    payload: {
      result: {
        projectInstanceId: "project-a",
        operationId,
        publicationRevision: 1,
        moves: [],
        deltas: [],
        projectionReplacements: [],
        projectionStatus: { status: "complete", expectedGraphPaths: [] },
      },
    },
  };
}

function item(operationId: string): ProjectEventStreamItem {
  return { kind: "event", event: event(operationId) };
}

const applied: ProjectEventConsumptionOutcome = { status: "applied" };

describe("project event ingress", () => {
  it("serializes the FIFO and removes queued work before a closed drain completes", async () => {
    const first = deferred<ProjectEventConsumptionOutcome>();
    const accepted: string[] = [];
    const consumer: ProjectEventConsumer = {
      acceptEvent: vi.fn((received) => {
        accepted.push(received.payload.result.operationId);
        return received.payload.result.operationId === "a"
          ? first.promise
          : Promise.resolve(applied);
      }),
    };
    const ingress = createProjectEventIngress(consumer, {
      capacity: 4,
      requestAuthoritativeSnapshot: vi.fn(async () => undefined),
    });

    expect(ingress.enqueue(item("a"))).toBe("accepted");
    expect(ingress.enqueue(item("b"))).toBe("accepted");
    await Promise.resolve();
    expect(accepted).toEqual(["a"]);

    const draining = ingress.closeAndDrain();
    first.resolve(applied);
    await expect(draining).resolves.toEqual({ status: "drained" });

    expect(accepted).toEqual(["a"]);
    expect(ingress.enqueue(item("c"))).toBe("closed");
  });

  it("drops the incremental tail on overflow and performs one recovery before reopening", async () => {
    const first = deferred<ProjectEventConsumptionOutcome>();
    const recovery = deferred<void>();
    const accepted: string[] = [];
    const recover = vi.fn(() => recovery.promise);
    const consumer: ProjectEventConsumer = {
      acceptEvent: vi.fn((received) => {
        accepted.push(received.payload.result.operationId);
        return received.payload.result.operationId === "a"
          ? first.promise
          : Promise.resolve(applied);
      }),
    };
    const ingress = createProjectEventIngress(consumer, {
      capacity: 1,
      requestAuthoritativeSnapshot: recover,
    });

    ingress.enqueue(item("a"));
    await Promise.resolve();
    ingress.enqueue(item("b"));
    expect(ingress.enqueue(item("c"))).toBe("overflowRecovery");
    expect(accepted).toEqual(["a"]);

    first.resolve(applied);
    await vi.waitFor(() => expect(recover).toHaveBeenCalledOnce());
    expect(accepted).toEqual(["a"]);

    recovery.resolve();
    await flushQueue();
    expect(ingress.enqueue(item("d"))).toBe("accepted");
    await flushQueue();
    expect(accepted).toEqual(["a", "d"]);
    await ingress.closeAndDrain();
  });

  it("drains one recovery when an overflowing consumer also requests a snapshot", async () => {
    const first = deferred<ProjectEventConsumptionOutcome>();
    const recovery = deferred<void>();
    const consumer = { acceptEvent: vi.fn(() => first.promise) };
    const recover = vi.fn(() => recovery.promise);
    const publishIssue = vi.fn();
    const ingress = createProjectEventIngress(consumer, {
      capacity: 1,
      requestAuthoritativeSnapshot: recover,
      publishIssue,
    });

    ingress.enqueue(item("a"));
    ingress.enqueue(item("b"));
    expect(ingress.enqueue(item("c"))).toBe("overflowRecovery");
    expect(recover).not.toHaveBeenCalled();
    const drained = vi.fn();
    const draining = ingress.closeAndDrain();
    void draining.then(drained);
    expect(ingress.closeAndDrain()).toBe(draining);

    first.resolve({ status: "recoveryRequested" });
    await vi.waitFor(() => expect(recover).toHaveBeenCalledOnce());
    expect(recover).toHaveBeenCalledWith("queueOverflow");
    expect(publishIssue).toHaveBeenCalledOnce();
    expect(consumer.acceptEvent).toHaveBeenCalledOnce();
    expect(drained).not.toHaveBeenCalled();

    recovery.resolve();
    await expect(draining).resolves.toEqual({ status: "drained" });
    expect(ingress.enqueue(item("d"))).toBe("closed");
  });

  it("waits for recovery requested by a consumer that rejects after closing", async () => {
    const first = deferred<ProjectEventConsumptionOutcome>();
    const recovery = deferred<void>();
    const recover = vi.fn(() => recovery.promise);
    const ingress = createProjectEventIngress(
      { acceptEvent: vi.fn(() => first.promise) },
      { requestAuthoritativeSnapshot: recover },
    );

    ingress.enqueue(item("a"));
    const drained = vi.fn();
    const draining = ingress.closeAndDrain();
    void draining.then(drained);
    first.reject(new Error("publication failed"));

    await vi.waitFor(() => expect(recover).toHaveBeenCalledWith("consumerRejected"));
    expect(drained).not.toHaveBeenCalled();
    recovery.resolve();
    await expect(draining).resolves.toEqual({ status: "drained" });
    expect(recover).toHaveBeenCalledOnce();
  });
});
