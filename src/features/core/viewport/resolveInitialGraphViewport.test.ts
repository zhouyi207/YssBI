// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { patchEditorViewStateViewport } from "./editorViewStateMemento";
import { resolveInitialGraphViewport } from "./resolveInitialGraphViewport";

describe("resolveInitialGraphViewport", () => {
  beforeEach(() => {
    localStorage.clear();
    useProjectIOStore.setState({ currentPath: "/projects/demo" });
  });

  it("prefers project editor view state memento", () => {
    patchEditorViewStateViewport("/projects/demo", "events/A.yssbi-event", {
      x: 50,
      y: 60,
      scale: 2,
    });

    expect(resolveInitialGraphViewport("events/A.yssbi-event")).toEqual({
      x: 50,
      y: 60,
      scale: 2,
    });
  });

  it("uses default viewport when memento is missing", () => {
    expect(resolveInitialGraphViewport("events/A.yssbi-event")).toEqual({ x: 0, y: 0, scale: 1 });
  });
});
