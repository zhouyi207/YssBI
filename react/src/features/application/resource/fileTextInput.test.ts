import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { MindEdit, MindSnapshot } from "@/shared/types/domain/mind";
import {
  clearResourceDocumentState,
  resourceKey,
  useResourceStore,
} from "@/features/core/resource";
import { startProjectLifecycle } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { createFileTextInput } from "./fileTextInput";
import {
  flushDocumentInputs,
  hasPendingDocumentInput,
  prepareDocumentInputDiscard,
  releaseDocumentInputs,
  resetDocumentInputs,
  retainDocumentInput,
} from "./documentInputs";

const saved: MindSnapshot = {
  projectInstanceId: "project-a",
  path: "minds/Plan.yssbi-mind",
  kind: "mind",
  version: { sessionId: "mind-session", revision: 0 },
  content: { rootId: "root", nodes: [{ id: "root", parentId: null, content: "Saved" }] },
  dirty: false,
};
const makeEdit = (content: string): MindEdit => ({ op: "set_content", nodeId: "root", content });

function setup() {
  let snapshot = structuredClone(saved);
  const edit = vi.fn(async (_path: string, edits: MindEdit[]) => {
    const content = edits[0];
    if (content.op !== "set_content") throw new Error("Expected a text edit");
    snapshot = {
      ...snapshot,
      version: { ...snapshot.version, revision: snapshot.version.revision + 1 },
      content: {
        ...snapshot.content,
        nodes: [{ ...snapshot.content.nodes[0], content: content.content }],
      },
      dirty: true,
    };
    return snapshot;
  });
  const actions = { edit, getSnapshot: () => snapshot };
  const acquire = () =>
    retainDocumentInput(saved.projectInstanceId, saved.path, "mind-session:root", () =>
      createFileTextInput(snapshot, snapshot.content.nodes[0].content, makeEdit, actions),
    );
  return { edit, acquire };
}

beforeEach(() => {
  resetDocumentInputs();
  startProjectLifecycle(saved.projectInstanceId);
  useResourceStore.getState().clear();
});
afterEach(resetDocumentInputs);

it("retains detached text and its captured version after a rejected submission", async () => {
  const { acquire, edit } = setup();
  const input = acquire();
  const unsubscribe = input.subscribe(vi.fn());
  input.change("Unsubmitted topic");
  unsubscribe();
  const restored = acquire();
  restored.sync("External change", { sessionId: "mind-session", revision: 2 }, makeEdit);
  expect(restored.getValue()).toBe("Unsubmitted topic");
  expect(hasPendingDocumentInput(saved.path)).toBe(true);
  edit.mockRejectedValueOnce(new Error("Version conflict"));
  await expect(flushDocumentInputs(saved.path)).rejects.toThrow("Version conflict");
  expect(edit).toHaveBeenCalledWith(saved.path, [makeEdit("Unsubmitted topic")], saved.version);
  expect(acquire().getValue()).toBe("Unsubmitted topic");
  expect(hasPendingDocumentInput(saved.path)).toBe(true);
  prepareDocumentInputDiscard(saved.path)();
  expect(restored.getValue()).toBe("External change");
  expect(hasPendingDocumentInput(saved.path)).toBe(false);
});

it("keeps newer text pending until it is separately submitted and ignores an older discard", async () => {
  const { acquire, edit } = setup();
  const input = acquire();
  let complete!: (result: MindSnapshot) => void;
  edit.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        complete = resolve;
      }),
  );
  input.change("First edit");
  const flushing = flushDocumentInputs(saved.path);
  input.change("Second edit");
  complete({ ...saved, version: { ...saved.version, revision: 1 }, dirty: true });
  await flushing;
  expect(input.getValue()).toBe("Second edit");
  expect(hasPendingDocumentInput(saved.path)).toBe(true);
  await flushDocumentInputs(saved.path);
  expect(edit).toHaveBeenLastCalledWith(saved.path, [makeEdit("Second edit")], {
    sessionId: "mind-session",
    revision: 1,
  });
  expect(hasPendingDocumentInput(saved.path)).toBe(false);
  input.change("Third edit");
  const discard = prepareDocumentInputDiscard(saved.path);
  input.change("Fourth edit");
  discard();
  expect(input.getValue()).toBe("Fourth edit");
  expect(hasPendingDocumentInput(saved.path)).toBe(true);
});

it("submits newer text once when concurrent flushes await the same preceding edit", async () => {
  const { acquire, edit } = setup();
  const input = acquire();
  const applyEdit = edit.getMockImplementation()!;
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  edit.mockImplementationOnce(async (path, edits) => {
    await gate;
    return applyEdit(path, edits);
  });

  input.change("First edit");
  const first = flushDocumentInputs(saved.path);
  input.change("Latest edit");
  const second = flushDocumentInputs(saved.path);
  const third = flushDocumentInputs(saved.path);
  release();
  await Promise.all([first, second, third]);

  expect(edit).toHaveBeenCalledTimes(2);
  expect(edit).toHaveBeenNthCalledWith(1, saved.path, [makeEdit("First edit")], saved.version);
  expect(edit).toHaveBeenNthCalledWith(2, saved.path, [makeEdit("Latest edit")], {
    sessionId: "mind-session",
    revision: 1,
  });
  expect(input.getValue()).toBe("Latest edit");
  expect(hasPendingDocumentInput(saved.path)).toBe(false);
});

it("releases retained inputs and ignores late settlements after file close or project reset", async () => {
  const { acquire, edit } = setup();
  const input = acquire();
  let complete!: (result: MindSnapshot) => void;
  edit.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        complete = resolve;
      }),
  );
  input.change("Pending before close");
  const flushing = flushDocumentInputs(saved.path);
  releaseDocumentInputs(saved.path);
  clearResourceDocumentState({ id: saved.path, kind: "mind" });
  complete({ ...saved, dirty: true });
  await flushing;
  expect(useResourceStore.getState().documents).toEqual({});
  expect(hasPendingDocumentInput(saved.path)).toBe(false);
  const reopened = acquire();
  expect(reopened).not.toBe(input);
  reopened.change("Pending before project replacement");
  resetDocumentInputs();
  startProjectLifecycle("project-b");
  useResourceStore.getState().clear();
  await reopened.flush();
  expect(edit).toHaveBeenCalledTimes(1);
  expect(useResourceStore.getState().documents).toEqual({});
  expect(hasPendingDocumentInput(saved.path)).toBe(false);
});

it("does not change successor dirty flags after text notifications replace the input owner", async () => {
  for (const phase of ["change-project", "change-release", "flush-project", "flush-release"]) {
    resetDocumentInputs();
    startProjectLifecycle(saved.projectInstanceId);
    useResourceStore.getState().clear();
    useResourceStore.getState().installFileSnapshot(saved);
    const { acquire, edit } = setup();
    const input = acquire();
    const flushing = phase.startsWith("flush");
    if (flushing) {
      input.change("pending text");
      edit.mockResolvedValueOnce({ ...saved, version: { ...saved.version, revision: 1 } });
    }
    const successor = {
      ...saved,
      projectInstanceId: phase.endsWith("project") ? "project-b" : saved.projectInstanceId,
      dirty: flushing,
      version: { sessionId: "successor", revision: 0 },
    };
    let installed = useResourceStore.getState().fileSnapshots.mind[saved.path];
    const stop = input.subscribe(() => {
      releaseDocumentInputs(saved.path);
      if (phase.endsWith("project")) startProjectLifecycle(successor.projectInstanceId);
      useResourceStore.getState().installFileSnapshot(successor);
      installed = useResourceStore.getState().fileSnapshots.mind[saved.path];
      retainDocumentInput(successor.projectInstanceId, saved.path, "successor:root", () =>
        createFileTextInput(successor, "successor", makeEdit, {
          edit,
          getSnapshot: () => successor,
        }),
      );
    });
    try {
      if (flushing) await input.flush();
      else input.change("old text");
      expect
        .soft(
          useResourceStore.getState().documents[resourceKey({ id: saved.path, kind: saved.kind })]
            ?.dirty,
          phase,
        )
        .toBe(successor.dirty);
      expect
        .soft(useResourceStore.getState().fileSnapshots.mind[saved.path], phase)
        .toBe(installed);
    } finally {
      stop();
    }
  }
});
