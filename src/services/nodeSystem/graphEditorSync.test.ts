import { invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import projectionWire from "@/tests/fixtures/node-system-contracts/editor-projection.json";
import { parseEditorGraphProjectionDto } from "@/shared/types/domain/editorProjectionParser";
import { makeGraphEditorSession } from "@/tests/helpers/editorProjectionFixtures";
import { clearGraphSyncBaselines, invokeGraphSync } from "./graphEditorSync";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import type { ParameterEditorDto } from "@/shared/types/domain/editorProjection";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const binding = {
  projectInstanceId: "project-sync",
  graphPath: projectionWire.graphPath,
  locale: "en-US",
};
const data = () =>
  makeGraphEditorSession(parseEditorGraphProjectionDto(structuredClone(projectionWire)));
const response = (update: Record<string, unknown>, changed = false) => ({
  ...binding,
  changed,
  resourceRevision: null,
  functionEditorProjection: null,
  update: { snapshotBytes: 10_000, ...update },
});

beforeEach(() => {
  vi.resetAllMocks();
  clearGraphSyncBaselines();
});
afterEach(() => useResourceStore.getState().clear());

it("retains parameter identities through transport splices and the existing Zustand store", async () => {
  const parameter = (key: string): ParameterEditorDto => ({
    key,
    display: { title: key, description: null },
    presentation: "detailPanel",
    valueType: { kind: "DataSeries", inner: { kind: "Scalar", inner: "Text" } },
    multiline: false,
    value: [],
    editor: {
      kind: "projectColumns",
      allowEmpty: true,
      available: true,
      unavailableReason: null,
      options: [],
      value: [],
    },
  });
  const initial = data();
  initial.projection.nodes[0].parameterGroups = [
    {
      key: "parameters",
      display: { title: "Parameters", description: null },
      parameters: ["keys", "sum", "mean"].map(parameter),
    },
  ];
  const nodeId = initial.projection.nodes[0].nodeId;
  vi.mocked(invoke).mockResolvedValueOnce(
    response({ kind: "snapshot", cursor: "first", data: initial }),
  );
  const first = await invokeGraphSync("load_project_graph", {}, binding);
  useResourceStore.getState().installGraphSession(binding.graphPath, first.data, { mode: "load" });
  const parameters = () =>
    useResourceStore.getState().graphEntities[binding.graphPath].nodes[nodeId].parameterGroups[0]
      .parameters;
  const before = parameters();
  let notifications = 0;
  let sumChanges = 0;
  const unsubscribe = useResourceStore.subscribe(() => {
    notifications++;
    if (parameters().find((p) => p.key === "sum") !== before[1]) sumChanges++;
  });
  for (const [baseCursor, cursor, deleteCount, values] of [
    ["first", "second", 0, [parameter("count")]],
    ["second", "third", 1, []],
  ] as const) {
    vi.mocked(invoke).mockResolvedValueOnce(
      response({
        kind: "delta",
        baseCursor,
        cursor,
        changes: [
          {
            kind: "splice",
            path: ["projection", "nodes", "0", "parameterGroups", "0", "parameters"],
            index: 1,
            deleteCount,
            values,
          },
        ],
      }),
    );
    const next = await invokeGraphSync("hydrate_editor_graph", {}, binding);
    useResourceStore.getState().installGraphSession(binding.graphPath, next.data);
    expect(parameters().find((p) => p.key === "sum")).toBe(before[1]);
    expect(parameters().find((p) => p.key === "mean")).toBe(before[2]);
  }
  unsubscribe();
  expect(notifications).toBe(2);
  expect(sumChanges).toBe(0);
  expect(parameters()).toEqual(before);
});

it.each(["cursor", "session", "operation"] as const)(
  "recovers a %s mismatch with a snapshot and never repeats an edit",
  async (fault) => {
    vi.mocked(invoke).mockResolvedValueOnce(
      response({ kind: "snapshot", cursor: "first", data: data() }),
    );
    const before = await invokeGraphSync("load_project_graph", {}, binding);
    const changes =
      fault === "session"
        ? [
            {
              kind: "set",
              path: ["editing", "version", "sessionId"],
              value: "00000000-0000-0000-0000-000000000099",
            },
          ]
        : fault === "operation"
          ? [
              {
                kind: "splice",
                path: ["projection", "nodes"],
                index: 0,
                deleteCount: 0,
                values: [],
                unexpected: true,
              },
            ]
          : [];
    vi.mocked(invoke)
      .mockResolvedValueOnce(
        response(
          {
            kind: "delta",
            baseCursor: fault === "cursor" ? "missing" : "first",
            cursor: "bad",
            changes,
          },
          true,
        ),
      )
      .mockResolvedValueOnce(response({ kind: "snapshot", cursor: "recovered", data: data() }));
    const result = await invokeGraphSync("edit_graph", { mutation: {} }, binding);
    expect(result.changed).toBe(true);
    expect(result.data).toEqual(before.data);
    expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual([
      "load_project_graph",
      "edit_graph",
      "hydrate_editor_graph",
    ]);
    expect(invoke).toHaveBeenLastCalledWith("hydrate_editor_graph", { ...binding, cursor: null });
  },
);

it("evicts cached projections by bytes and reads them again without an expired cursor", async () => {
  for (const projectInstanceId of ["first", "second", "third"]) {
    const scoped = { ...binding, projectInstanceId };
    vi.mocked(invoke).mockResolvedValueOnce({
      ...response({
        kind: "snapshot",
        cursor: projectInstanceId,
        snapshotBytes: 16 * 1024 * 1024,
        data: data(),
      }),
      ...scoped,
    });
    await invokeGraphSync("load_project_graph", {}, scoped);
  }
  const scoped = { ...binding, projectInstanceId: "first" };
  vi.mocked(invoke).mockResolvedValueOnce({
    ...response({ kind: "snapshot", cursor: "refreshed", data: data() }),
    ...scoped,
  });
  await invokeGraphSync("hydrate_editor_graph", {}, scoped);
  expect(invoke).toHaveBeenLastCalledWith("hydrate_editor_graph", { ...scoped, cursor: null });
});

it("applies a validated delta while retaining untouched projection objects", async () => {
  vi.mocked(invoke).mockResolvedValueOnce(
    response({ kind: "snapshot", cursor: "first", data: data() }),
  );
  const first = await invokeGraphSync("load_project_graph", binding, binding);
  vi.mocked(invoke).mockResolvedValueOnce(
    response({
      kind: "delta",
      baseCursor: "first",
      cursor: "next",
      changes: [
        { kind: "set", path: ["projection", "nodes", "0", "display", "title"], value: "Updated" },
      ],
    }),
  );
  const next = await invokeGraphSync("hydrate_editor_graph", binding, binding);
  expect(next.data.projection.nodes[0].display.title).toBe("Updated");
  expect(first.data.projection.nodes[0].display.title).not.toBe("Updated");
  expect(next.data.document).toBe(first.data.document);
  expect(next.data.projection.nodes[0].ports).toBe(first.data.projection.nodes[0].ports);
  expect(invoke).toHaveBeenLastCalledWith("hydrate_editor_graph", { ...binding, cursor: "first" });
});

it.each(["semantic hash", "duplicate node"])(
  "recovers a %s mismatch after immutable baseline branches have been cached",
  async (fault) => {
    const initial = data();
    vi.mocked(invoke)
      .mockResolvedValueOnce(response({ kind: "snapshot", cursor: "first", data: initial }))
      .mockResolvedValueOnce(
        response({ kind: "delta", baseCursor: "first", cursor: "warm", changes: [] }),
      )
      .mockResolvedValueOnce(
        response(
          {
            kind: "delta",
            baseCursor: "warm",
            cursor: "bad",
            changes:
              fault === "semantic hash"
                ? [
                    {
                      kind: "set",
                      path: ["projection", "basis", "semanticInputHash"],
                      value: "f".repeat(64),
                    },
                  ]
                : [
                    {
                      kind: "splice",
                      path: ["projection", "nodes"],
                      index: 0,
                      deleteCount: 0,
                      values: [initial.projection.nodes[0]],
                    },
                  ],
          },
          true,
        ),
      )
      .mockResolvedValueOnce(response({ kind: "snapshot", cursor: "recovered", data: data() }));
    const before = await invokeGraphSync("load_project_graph", {}, binding);
    expect(Object.isFrozen(before.data.projection.nodes[0].position)).toBe(true);
    const warm = await invokeGraphSync("hydrate_editor_graph", {}, binding);
    expect(warm.data.resultState).toBe(before.data.resultState);
    const recovered = await invokeGraphSync("edit_graph", { mutation: {} }, binding);
    expect(recovered.data).toEqual(before.data);
    expect(recovered.data.projection.basis.semanticInputHash).toBe(
      before.data.resultState.semanticInputHash,
    );
    expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual([
      "load_project_graph",
      "hydrate_editor_graph",
      "edit_graph",
      "hydrate_editor_graph",
    ]);
    expect(invoke).toHaveBeenLastCalledWith("hydrate_editor_graph", { ...binding, cursor: null });
  },
);

it("recovers a bad mutation delta through one read without replaying the write", async () => {
  const initial = data();
  const recovered = data();
  recovered.editing.version.revision = "1";
  recovered.editing.dirty = true;
  vi.mocked(invoke)
    .mockResolvedValueOnce(response({ kind: "snapshot", cursor: "first", data: initial }))
    .mockResolvedValueOnce(
      response(
        {
          kind: "delta",
          baseCursor: "first",
          cursor: "broken",
          changes: [
            { kind: "set", path: ["editing", "version", "revision"], value: "1" },
            { kind: "remove", path: ["document", "missing"] },
          ],
        },
        true,
      ),
    )
    .mockResolvedValueOnce(response({ kind: "snapshot", cursor: "recovered", data: recovered }));
  const before = await invokeGraphSync("load_project_graph", binding, binding);
  const result = await invokeGraphSync("edit_graph", { ...binding, mutation: {} }, binding);
  expect(result.changed).toBe(true);
  expect(result.data.editing).toEqual(recovered.editing);
  expect(before.data.editing.version.revision).toBe("0");
  expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual([
    "load_project_graph",
    "edit_graph",
    "hydrate_editor_graph",
  ]);
  expect(invoke).toHaveBeenLastCalledWith("hydrate_editor_graph", { ...binding, cursor: null });
});

it("recovers a lost save reply from its receipt while retaining later dirty edits", async () => {
  const current = data();
  const version = { ...current.editing.version };
  current.editing.version.revision = "2";
  current.editing.dirty = true;
  const operationId = crypto.randomUUID();
  const identity = { projectInstanceId: binding.projectInstanceId, graphPath: binding.graphPath };
  vi.mocked(invoke)
    .mockRejectedValueOnce(new Error("reply lost after commit"))
    .mockResolvedValueOnce({
      ...identity,
      operationId,
      requestVersion: version,
      committedVersion: { ...version, revision: "1" },
      command: "save",
      changed: false,
    })
    .mockResolvedValueOnce(response({ kind: "snapshot", cursor: "current", data: current }));
  const result = await invokeGraphSync("save_project_graph", { version, operationId }, binding);
  expect(result.resourceRevision).toBe(1);
  expect(result.data.editing.version.revision).toBe("2");
  expect(result.data.editing.dirty).toBe(true);
  expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual([
    "save_project_graph",
    "get_graph_edit_receipt",
    "hydrate_editor_graph",
  ]);
  expect(invoke).toHaveBeenNthCalledWith(2, "get_graph_edit_receipt", {
    ...identity,
    operationId,
    version,
  });
});

it("does not treat an unknown receipt or a business rejection as a successful write", async () => {
  const version = data().editing.version;
  const args = { version, operationId: crypto.randomUUID() };
  vi.mocked(invoke)
    .mockRejectedValueOnce(new Error("reply unavailable"))
    .mockResolvedValueOnce(null);
  await expect(invokeGraphSync("edit_graph", args, binding)).rejects.toThrow("reply unavailable");
  expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual([
    "edit_graph",
    "get_graph_edit_receipt",
  ]);
  vi.mocked(invoke)
    .mockClear()
    .mockRejectedValueOnce({ code: "duplicate_operation", details: null, incidentId: null });
  await expect(invokeGraphSync("edit_graph", args, binding)).rejects.toMatchObject({
    code: "duplicate_operation",
  });
  expect(invoke).toHaveBeenCalledTimes(1);
});
