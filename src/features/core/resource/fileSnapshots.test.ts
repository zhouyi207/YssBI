import { expect, it, vi } from "vitest";
import { useResourceStore } from "./resourceStore";
import type { MindSnapshot } from "@/shared/types/domain/mind";

it("retains Mind node identities when a complete snapshot reorders unchanged nodes", () => {
  useResourceStore.getState().clear();
  const snapshot: MindSnapshot = {
    projectInstanceId: "project-a",
    path: "minds/Plan.yssbi-mind",
    kind: "mind",
    version: { sessionId: "session-a", revision: 1 },
    dirty: false,
    content: {
      rootId: "root",
      nodes: [
        { id: "root", parentId: null, content: "Plan" },
        {
          id: "a",
          parentId: "root",
          content: "A",
          reference: { kind: "resource", path: "docs/A.md" },
        },
        { id: "b", parentId: "root", content: "B" },
      ],
    },
  };
  const store = useResourceStore.getState();
  store.installFileSnapshot(snapshot);
  const before = useResourceStore.getState();
  const incoming = structuredClone(snapshot);
  incoming.version.revision++;
  incoming.content.nodes = [
    incoming.content.nodes[0],
    incoming.content.nodes[2],
    incoming.content.nodes[1],
  ];
  const notified = vi.fn();
  const stop = useResourceStore.subscribe(notified);
  try {
    store.setSnapshot({
      resources: [],
      fileSnapshots: { mind: { [snapshot.path]: incoming }, doc: {} },
    });
    const installed = useResourceStore.getState().fileSnapshots.mind[snapshot.path];
    expect(notified).toHaveBeenCalledOnce();
    expect(installed.content.nodes[1]).toBe(
      before.fileSnapshots.mind[snapshot.path].content.nodes[2],
    );
    expect(installed.content.nodes[2]).toBe(
      before.fileSnapshots.mind[snapshot.path].content.nodes[1],
    );
    expect(before.fileSnapshots.mind[snapshot.path].content.nodes.map((node) => node.id)).toEqual([
      "root",
      "a",
      "b",
    ]);
  } finally {
    stop();
  }
});

it("shares unchanged file branches and ignores equal or outdated installations", () => {
  useResourceStore.getState().clear();
  const snapshot: MindSnapshot = {
    projectInstanceId: "project-a",
    path: "minds/Plan.yssbi-mind",
    kind: "mind",
    version: { sessionId: "session-a", revision: 1 },
    dirty: false,
    content: {
      rootId: "root",
      nodes: [
        { id: "root", parentId: null, content: "Plan" },
        { id: "child", parentId: "root", content: "Child" },
      ],
    },
  };
  useResourceStore.getState().installFileSnapshot(snapshot);
  const before = useResourceStore.getState();
  const notified = vi.fn();
  const stop = useResourceStore.subscribe(notified);
  try {
    expect(useResourceStore.getState().installFileSnapshot(structuredClone(snapshot))).toBe(true);
    useResourceStore.getState().setSnapshot({
      resources: Object.values(before.resources),
      documents: before.documents,
      fileSnapshots: structuredClone(before.fileSnapshots),
    });
    useResourceStore.getState().removeFileSnapshot("mind", "minds/Absent.yssbi-mind");
    expect.soft(notified).not.toHaveBeenCalled();
    expect.soft(useResourceStore.getState()).toBe(before);
    notified.mockClear();
    const changed = structuredClone(snapshot);
    changed.version.revision = 2;
    changed.content.nodes[1].content = "Updated";
    expect(useResourceStore.getState().installFileSnapshot(changed)).toBe(true);
    const installed = useResourceStore.getState().fileSnapshots.mind[snapshot.path];
    expect(notified).toHaveBeenCalledOnce();
    expect
      .soft(installed.content.nodes[0])
      .toBe(before.fileSnapshots.mind[snapshot.path].content.nodes[0]);
    expect(before.fileSnapshots.mind[snapshot.path].content.nodes[1].content).toBe("Child");
    expect(useResourceStore.getState().installFileSnapshot(snapshot)).toBe(false);
    expect(useResourceStore.getState().fileSnapshots.mind[snapshot.path]).toBe(installed);
  } finally {
    stop();
  }
});
