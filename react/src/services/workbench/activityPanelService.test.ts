import { ProjectService } from "@/services/project/projectService";
import { projectIndexSnapshotFixture } from "@/tests/helpers/activityPanelFixture";
import { expect, it, vi } from "vitest";
import { getActivityPanelDocument } from "./activityPanelService";
import {
  parseActivityPanelDocument,
  parseActivityPanelUpdate,
} from "@/shared/types/dto/activityPanel";
import { activityPanelFixture, categoryFixture } from "@/tests/helpers/activityPanelFixture";
import type { ActivityFileItem, ActivityPanelRow } from "@/shared/types/domain/activityPanel";
import type { ProjectIndexRow } from "@/shared/types/domain/project";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@/services/ipc", async (original) => ({
  ...(await original<typeof import("@/services/ipc")>()),
  invokeCommand: invoke,
}));

it("accepts concrete file and database items in project snapshots while keeping other panels scoped", async () => {
  const files: ActivityFileItem[] = [
    { kind: "event_graph", path: "events/Main.yssbi-event", name: "Main" },
    { kind: "function_graph", path: "functions/Calculate.yssbi-function", name: "Calculate" },
    { kind: "chart", path: "charts/Sales.yssbi-chart", name: "Sales" },
    { kind: "mind", path: "minds/Plan.yssbi-mind", name: "Plan" },
    { kind: "doc", path: "docs/Report.md", name: "Report" },
  ];
  const rows: ActivityPanelRow[] = files.flatMap((item) => [
    categoryFixture(`project.${item.kind}s`, item.kind, 0, true),
    { id: `${item.kind}:${item.path}`, depth: 1, kind: "item", item },
  ]);
  rows.push(categoryFixture("project.data", "Data", 0, true), {
    id: "database:sales",
    depth: 1,
    kind: "item",
    item: { kind: "database", id: "sales", name: "Sales", resourcePath: "data/sales.yssdb" },
  });
  const document = activityPanelFixture("project", rows);
  invoke.mockReset();
  invoke.mockResolvedValueOnce({ kind: "snapshot", cursor: "p1", document });

  const snapshot = await getActivityPanelDocument(
    "project",
    { projectInstanceId: "project-1" },
    "en-US",
  );

  expect(snapshot.document).toBe(document);
  expect(invoke).toHaveBeenCalledOnce();
  for (const panelId of ["nodes", "commands", "plugins"] as const) {
    expect(parseActivityPanelDocument(activityPanelFixture(panelId, rows))).toBeNull();
  }
});

it("validates conversation documents and applies rename and order patches without accepting duplicate identities", async () => {
  const row = {
    id: "conversation:first",
    depth: 0,
    kind: "item",
    item: { kind: "conversation", sessionId: "first", title: "First", lastOpenedAt: 1 },
  } satisfies ActivityPanelRow;
  const second = {
    ...row,
    id: "conversation:second",
    item: { ...row.item, sessionId: "second", title: "Second" },
  } satisfies ActivityPanelRow;
  const document = {
    ...activityPanelFixture(
      "assistant",
      [row, second],
      [
        {
          id: "newConversation",
          icon: "add",
          label: { key: "panel.assistantNewConversation" },
        },
      ],
    ),
    projectInstanceId: "project-1",
  };
  invoke.mockReset().mockResolvedValueOnce({ kind: "snapshot", cursor: "a1", document });
  const first = await getActivityPanelDocument(
    "assistant",
    { projectInstanceId: "project-1" },
    "en-US",
  );
  expect(first.document).toBe(document);
  const renamed = { ...second.item, title: "Renamed", lastOpenedAt: 2 };
  invoke.mockResolvedValueOnce({
    kind: "patch",
    baseCursor: "a1",
    cursor: "a2",
    patch: {},
    operations: [
      { op: "update", id: second.id, patch: { item: renamed } },
      { op: "move", id: second.id, afterId: null },
    ],
  });
  const updated = await getActivityPanelDocument(
    "assistant",
    { projectInstanceId: "project-1" },
    "en-US",
    first,
  );
  expect(updated.document.rows).toEqual([{ ...second, item: renamed }, row]);
  expect(first.document.rows).toEqual([row, second]);
  for (const item of [
    { ...row.item, lastOpenedAt: -1 },
    { ...row.item, lastOpenedAt: Number.MAX_SAFE_INTEGER },
    { kind: "conversation", sessionId: "missing-metadata" },
    { ...row.item, sessionId: "" },
  ])
    expect(parseActivityPanelDocument({ ...document, rows: [{ ...row, item }] })).toBeNull();
  expect(
    parseActivityPanelDocument({ ...document, rows: [row, { ...row, id: "duplicate" }] }),
  ).toBeNull();
  expect(parseActivityPanelDocument({ ...document, panelId: "project" })).toBeNull();
});

it("publishes a newly created mind through a coherent index and sidebar insert patch", async () => {
  const mind = {
    kind: "mind" as const,
    path: "minds/New Mind Map.yssbi-mind",
    name: "New Mind Map",
    revision: 0,
  };
  const index: ProjectIndexRow = {
    projectInstanceId: "project-1",
    publicationRevision: 1,
    projectName: "Project",
    exportTime: "",
    eventGraphs: [],
    functionGraphs: [],
    charts: [],
    minds: [],
    docs: [],
    databases: [],
  };
  const initial = projectIndexSnapshotFixture(index);
  const category = { ...categoryFixture("project.minds", "Minds", 0, true), count: 0 };
  const previous = {
    ...initial.activityPanels,
    project: {
      ...initial.activityPanels.project,
      document: activityPanelFixture("project", [category]),
    },
  };
  const row: ActivityPanelRow = {
    id: `mind:${mind.path}`,
    depth: 1,
    kind: "item",
    item: { kind: mind.kind, path: mind.path, name: mind.name },
  };
  invoke.mockReset();
  invoke.mockResolvedValue({
    index: { ...index, publicationRevision: 2, minds: [mind] },
    activityPanels: {
      project: {
        kind: "patch",
        baseCursor: previous.project.cursor,
        cursor: "p2",
        patch: { publicationRevision: 2 },
        operations: [
          { op: "update", id: category.id, patch: { count: 1 } },
          { op: "insert", afterId: category.id, row },
        ],
      },
      nodes: {
        kind: "patch",
        baseCursor: previous.nodes.cursor,
        cursor: "n2",
        patch: { publicationRevision: 2 },
        operations: [],
      },
    },
  });

  const snapshot = await ProjectService.getProjectIndex("project-1", "en-US", previous);

  expect(snapshot.index.minds).toEqual([mind]);
  expect(snapshot.activityPanels.project.document.rows).toEqual([{ ...category, count: 1 }, row]);
  expect(snapshot.activityPanels.project.document.publicationRevision).toBe(2);
  expect(snapshot.activityPanels.nodes.document.publicationRevision).toBe(2);
  expect(invoke).toHaveBeenCalledOnce();
});

it("applies ID patches atomically, preserves unchanged references and recovers a lost baseline once", async () => {
  const summary: ActivityPanelRow = {
    id: "summary",
    depth: 1,
    kind: "message",
    label: { text: "Old result" },
    description: null,
  };
  const document = activityPanelFixture("nodes", [
    categoryFixture("project.eventGraphs", "Server title", 0, true),
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
        { op: "insert", afterId: "project.eventGraphs", row: { ...summary, id: "inserted" } },
        { op: "move", id: "kept", afterId: "project.eventGraphs" },
      ],
    },
    second,
  );
  expect(reordered?.document.rows.map((row) => row.id)).toEqual([
    "project.eventGraphs",
    "kept",
    "inserted",
  ]);
  expect(second.document.rows.map((row) => row.id)).toEqual([
    "project.eventGraphs",
    "summary",
    "kept",
  ]);

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
    eventGraphs: [],
    functionGraphs: [],
    minds: [],
    docs: [],
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
            categoryFixture("project.eventGraphs", "Events", 0, true),
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
