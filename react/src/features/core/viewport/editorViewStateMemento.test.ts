// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import {
  editorViewStateStorageKey,
  loadEditorViewStateMemento,
  patchEditorViewStateViewport,
  readEditorViewStateViewport,
  remapEditorViewStateGraphPath,
  saveEditorViewStateMemento,
} from "./editorViewStateMemento";

describe("editorViewStateMemento", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("persists viewport per project path and graph path", () => {
    patchEditorViewStateViewport("/projects/demo", "events/A.yssbi-event", {
      x: 10,
      y: 20,
      scale: 1.25,
    });

    expect(readEditorViewStateViewport("/projects/demo", "events/A.yssbi-event")).toEqual({
      x: 10,
      y: 20,
      scale: 1.25,
    });
  });

  it("isolates mementos by project path", () => {
    patchEditorViewStateViewport("/projects/a", "events/A.yssbi-event", { x: 1, y: 2, scale: 1 });
    patchEditorViewStateViewport("/projects/b", "events/A.yssbi-event", { x: 9, y: 8, scale: 2 });

    expect(readEditorViewStateViewport("/projects/a", "events/A.yssbi-event")).toEqual({
      x: 1,
      y: 2,
      scale: 1,
    });
    expect(readEditorViewStateViewport("/projects/b", "events/A.yssbi-event")).toEqual({
      x: 9,
      y: 8,
      scale: 2,
    });
  });

  it("remaps graph paths within a project memento", () => {
    saveEditorViewStateMemento("/projects/demo", {
      "events/old.yssbi-event": { x: 3, y: 4, scale: 1 },
    });

    remapEditorViewStateGraphPath(
      "/projects/demo",
      "events/old.yssbi-event",
      "events/new.yssbi-event",
    );

    const memento = loadEditorViewStateMemento("/projects/demo");
    expect(memento["events/old.yssbi-event"]).toBeUndefined();
    expect(memento["events/new.yssbi-event"]).toEqual({ x: 3, y: 4, scale: 1 });
  });

  it("uses encoded project path in storage key", () => {
    patchEditorViewStateViewport("D:/My Projects/demo", "g1", { x: 0, y: 0, scale: 1 });
    expect(localStorage.getItem(editorViewStateStorageKey("D:/My Projects/demo"))).toContain("g1");
  });
});
