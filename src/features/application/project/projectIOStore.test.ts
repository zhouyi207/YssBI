import { afterEach, describe, expect, it } from "vitest";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { captureProjectReadContext, useProjectIOStore } from "./projectIOStore";

afterEach(() => {
  clearProjectLifecycle();
  useProjectIOStore.setState({ projectInstanceId: null });
});

describe("project read context", () => {
  it("rejects reads while activation and the displayed projection disagree", () => {
    expect(captureProjectReadContext(null)).toBeNull();
    startProjectLifecycle("project-a");
    useProjectIOStore.setState({ projectInstanceId: "project-a" });
    const prior = captureProjectReadContext("project-a")!;
    expect(prior.isCurrent()).toBe(true);

    startProjectLifecycle("project-b");
    expect(prior.isCurrent()).toBe(false);
    expect(captureProjectReadContext("project-a")).toBeNull();
    expect(captureProjectReadContext("project-b")).toBeNull();

    useProjectIOStore.setState({ projectInstanceId: "project-b" });
    const current = captureProjectReadContext("project-b")!;
    expect(current.projectInstanceId).toBe("project-b");
    expect(current.isCurrent()).toBe(true);
    useProjectIOStore.setState({ projectInstanceId: null });
    expect(current.isCurrent()).toBe(false);
  });

  it("invalidates pending reads after clearing and restarting the same project identity", async () => {
    startProjectLifecycle("project-a");
    useProjectIOStore.setState({ projectInstanceId: "project-a" });
    const prior = captureProjectReadContext("project-a")!;
    let settle!: () => void;
    const pending = new Promise<void>((resolve) => {
      settle = resolve;
    }).then(() => prior.isCurrent());

    clearProjectLifecycle();
    expect(captureProjectReadContext("project-a")).toBeNull();
    startProjectLifecycle("project-a");
    expect(captureProjectReadContext("project-a")?.isCurrent()).toBe(true);
    settle();
    expect(await pending).toBe(false);
  });
});
