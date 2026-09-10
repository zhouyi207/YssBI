import { ProjectService } from "@/services/project/projectService";
import { projectIndexSnapshotFixture } from "@/tests/helpers/activityPanelFixture";
import { expect, it, vi } from "vitest";
import { getActivityPanelDocument } from "./activityPanelService";
import {
  parseActivityPanelDocument,
  parseActivityPanelUpdate,
} from "@/shared/types/dto/activityPanel";
import { activityPanelFixture, categoryFixture } from "@/tests/helpers/activityPanelFixture";
import type { ActivityPanelRow } from "@/shared/types/domain/activityPanel";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@/services/ipc", async (original) => ({
  ...(await original<typeof import("@/services/ipc")>()),
  invokeCommand: invoke,
}));

it("applies ID patches atomically, preserves unchanged references and recovers a lost baseline once", async () => {
  const summary: ActivityPanelRow = {
    id: "summary",
    depth: 1,
    kind: "message",
    label: { text: "Old result" },
    description: null,
  };
  const document = activityPanelFixture("nodes", [
    categoryFixture("project.events", "Server title", 0, true),
    summary,
    { ...summary, id: "kept" },
  ]);
  invoke.mockResolvedValue({ kind: "snapshot", cursor: "c1", document });
  const first = await getActivityPanelDocument(
    "nodes",
    { projectInstanceId: "project-1" },
    "en-US",
  );
  expect(first.document).toBe(document);
  expect(invoke).toHaveBeenCalledWith("get_activity_panel_document", {
    panelId: "nodes",
    projectInstanceId: "project-1",
    locale: "en-US",
    cursor: null,
  });
  const change = {
    kind: "patch",
    baseCursor: "c1",
    cursor: "c2",
    patch: { publicationRevision: 2 },
    operations: [{ op: "update", id: "summary", patch: { label: { text: "New result" } } }],
  };
  invoke.mockResolvedValueOnce(change);
  const second = await getActivityPanelDocument(
    "nodes",
    { projectInstanceId: "project-1" },
    "en-US",
    first,
  );
  expect(invoke).toHaveBeenLastCalledWith("get_activity_panel_document", {
    panelId: "nodes",
    projectInstanceId: "project-1",
    locale: "en-US",
    cursor: "c1",
  });
  expect(second.document.rows[1]).toMatchObject({ label: { text: "New result" } });
  expect(second.document.rows[0]).toBe(first.document.rows[0]);
  expect(second.document.rows[2]).toBe(first.document.rows[2]);
  expect(first.document.rows[1]).toBe(summary);
  expect(summary.label).toEqual({ text: "Old result" });
  expect(
    parseActivityPanelUpdate(
      { ...change, operations: [...change.operations, { op: "remove", id: "missing" }] },
      first,
    ),
  ).toBeNull();
  expect(
    parseActivityPanelUpdate(
      { kind: "patch", baseCursor: "c2", cursor: "c2", patch: {}, operations: [] },
      second,
    ),
  ).toBe(second);

  const reordered = parseActivityPanelUpdate(
    {
      kind: "patch",
      baseCursor: "c2",
      cursor: "c3",
      patch: {},
      operations: [
        { op: "remove", id: "summary" },
        { op: "insert", afterId: "project.events", row: { ...summary, id: "inserted" } },
        { op: "move", id: "kept", afterId: "project.events" },
      ],
    },
    second,
  );
  expect(reordered?.document.rows.map((row) => row.id)).toEqual([
    "project.events",
    "kept",
    "inserted",
  ]);
  expect(second.document.rows.map((row) => row.id)).toEqual(["project.events", "summary", "kept"]);

  invoke.mockResolvedValueOnce({ ...change, baseCursor: "missing" }).mockResolvedValueOnce({
    kind: "snapshot",
    cursor: "reset",
    document: { ...document, publicationRevision: 3 },
  });
  expect(
    (await getActivityPanelDocument("nodes", { projectInstanceId: "project-1" }, "en-US", second))
      .cursor,
  ).toBe("reset");
  expect(invoke).toHaveBeenLastCalledWith("get_activity_panel_document", {
    panelId: "nodes",
    projectInstanceId: "project-1",
    locale: "en-US",
    cursor: null,
  });
  await expect(
    getActivityPanelDocument("nodes", { projectInstanceId: "project-2" }, "en-US"),
  ).rejects.toThrow();
  expect(
    parseActivityPanelDocument({ ...document, rows: [...document.rows, ...document.rows] }),
  ).toBeNull();
  expect(
    parseActivityPanelDocument({
      ...document,
      rows: [{ ...document.rows[0], html: "<script />" }],
    }),
  ).toBeNull();
});

it("parses a coherent index with panel operations and recovers the entire batch if one cursor is invalid", async () => {
  const index = {
    projectInstanceId: "project-1",
    publicationRevision: 1,
    projectName: "Project",
    exportTime: "",
    graphs: [],
    charts: [],
    databases: [],
  };
  const initial = projectIndexSnapshotFixture(index);
  const first = {
    ...initial,
    activityPanels: {
      ...initial.activityPanels,
      project: {
        cursor: "p1",
        document: {
          ...initial.activityPanels.project.document,
          rows: [
            categoryFixture("project.events", "Events", 0, true),
            {
              id: "summary",
              depth: 1,
              kind: "message" as const,
              label: { text: "Old" },
              description: null,
            },
          ],
        },
      },
    },
  };
  const patches = {
    index: { ...index, publicationRevision: 2 },
    activityPanels: {
      project: {
        kind: "patch",
        baseCursor: "p1",
        cursor: "p2",
        patch: { publicationRevision: 2 },
        operations: [{ op: "update", id: "summary", patch: { label: { text: "Updated" } } }],
      },
      nodes: {
        kind: "patch",
        baseCursor: initial.activityPanels.nodes.cursor,
        cursor: "n2",
        patch: { publicationRevision: 2 },
        operations: [],
      },
    },
  };
  invoke.mockReset();
  invoke.mockResolvedValueOnce(patches);
  const updated = await ProjectService.getProjectIndex("project-1", "en-US", first.activityPanels);
  expect(updated.index.publicationRevision).toBe(2);
  expect(updated.activityPanels.project.document.rows[1]).toMatchObject({
    label: { text: "Updated" },
  });
  expect(updated.activityPanels.project.document.rows[0]).toBe(
    first.activityPanels.project.document.rows[0],
  );
  expect(first.activityPanels.project.document.rows[1]).toMatchObject({ label: { text: "Old" } });
  expect(invoke).toHaveBeenCalledOnce();
  expect(invoke).toHaveBeenCalledWith("get_project_index", {
    projectInstanceId: "project-1",
    locale: "en-US",
    activityPanels: [
      { panelId: "project", cursor: "p1" },
      { panelId: "nodes", cursor: initial.activityPanels.nodes.cursor },
    ],
  });
  const recovered = projectIndexSnapshotFixture({ ...index, publicationRevision: 3 });
  invoke
    .mockResolvedValueOnce({
      ...patches,
      activityPanels: {
        ...patches.activityPanels,
        nodes: { ...patches.activityPanels.nodes, baseCursor: "missing" },
      },
    })
    .mockResolvedValueOnce({
      index: recovered.index,
      activityPanels: Object.fromEntries(
        Object.entries(recovered.activityPanels).map(([id, snapshot]) => [
          id,
          { kind: "snapshot", ...snapshot },
        ]),
      ),
    });
  expect(
    (await ProjectService.getProjectIndex("project-1", "en-US", first.activityPanels)).index
      .publicationRevision,
  ).toBe(3);
  expect(invoke).toHaveBeenLastCalledWith("get_project_index", {
    projectInstanceId: "project-1",
    locale: "en-US",
    activityPanels: [
      { panelId: "project", cursor: null },
      { panelId: "nodes", cursor: null },
    ],
  });
  expect(first.activityPanels.project.document.rows[1]).toMatchObject({ label: { text: "Old" } });
});
