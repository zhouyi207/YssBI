import { expect, it, vi } from "vitest";
import { getActivityPanelDocument } from "./activityPanelService";
import {
  parseActivityPanelDocument,
  parseActivityPanelUpdate,
} from "@/shared/types/dto/activityPanel";
import { activityPanelFixture, categoryFixture } from "@/tests/helpers/activityPanelFixture";
import type { ActivityPanelRow } from "@/shared/types/domain/activityPanel";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@/services/ipc", () => ({ invokeCommand: invoke }));

it("applies ID patches atomically, preserves unchanged references and recovers a lost baseline once", async () => {
  const summary: ActivityPanelRow = {
    id: "summary",
    depth: 1,
    kind: "message",
    label: { text: "Old result" },
    description: null,
  };
  const document = activityPanelFixture("project", [
    categoryFixture("project.events", "Server title", 0, true),
    summary,
    { ...summary, id: "kept" },
  ]);
  invoke.mockResolvedValue({ kind: "snapshot", cursor: "c1", document });
  const first = await getActivityPanelDocument(
    "project",
    { projectInstanceId: "project-1" },
    "en-US",
  );
  expect(first.document).toBe(document);
  expect(invoke).toHaveBeenCalledWith("get_activity_panel_document", {
    panelId: "project",
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
    "project",
    { projectInstanceId: "project-1" },
    "en-US",
    first,
  );
  expect(invoke).toHaveBeenLastCalledWith("get_activity_panel_document", {
    panelId: "project",
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
    (await getActivityPanelDocument("project", { projectInstanceId: "project-1" }, "en-US", second))
      .cursor,
  ).toBe("reset");
  expect(invoke).toHaveBeenLastCalledWith("get_activity_panel_document", {
    panelId: "project",
    projectInstanceId: "project-1",
    locale: "en-US",
    cursor: null,
  });
  await expect(
    getActivityPanelDocument("project", { projectInstanceId: "project-2" }, "en-US"),
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
