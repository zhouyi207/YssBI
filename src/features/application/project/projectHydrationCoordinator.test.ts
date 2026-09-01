import { describe, expect, it, vi } from "vitest";
import {
  createProjectHydrationCoordinator,
  type ProjectHydrationIdentity,
} from "./projectHydrationCoordinator";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

describe("project hydration coordinator", () => {
  it("rejects stale project completion after a replacement epoch", async () => {
    let identity: ProjectHydrationIdentity = { projectInstanceId: "project-a", epoch: 1 };
    const projectA = deferred<{ projectInstanceId: string }>();
    const projectB = deferred<{ projectInstanceId: string }>();
    const published: string[] = [];

    const coordinator = createProjectHydrationCoordinator({
      captureProjectIdentity: () => identity,
      loadProjectIndex: (request) =>
        request.projectInstanceId === "project-a" ? projectA.promise : projectB.promise,
      publishProjectSnapshot: vi.fn((snapshot) => {
        published.push(snapshot.projectInstanceId);
      }),
    });

    const loadingA = coordinator.loadCurrentProject();
    identity = { projectInstanceId: "project-b", epoch: 2 };
    coordinator.replaceProject();
    const loadingB = coordinator.loadCurrentProject();

    projectB.resolve({ projectInstanceId: "project-b" });
    expect(await loadingB).toEqual({ status: "published" });

    projectA.resolve({ projectInstanceId: "project-a" });
    expect(await loadingA).toEqual({ status: "stale" });
    expect(published).toEqual(["project-b"]);
  });
});
