import { invoke } from "@tauri-apps/api/core";
import { beforeEach, expect, it, vi } from "vitest";
import projectionWire from "@/tests/fixtures/node-system-contracts/editor-projection.json";
import { parseEditorGraphProjectionDto } from "@/shared/types/dto/editorProjectionParser";
import { makeGraphEditorSession } from "@/tests/helpers/editorProjectionFixtures";
import { clearGraphSyncBaselines, invokeGraphSync } from "./graphEditorSync";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const binding = {
  projectInstanceId: "project-sync",
  graphPath: projectionWire.graphPath,
  locale: "en-US",
};
const data = () =>
  makeGraphEditorSession(parseEditorGraphProjectionDto(structuredClone(projectionWire)));
const response = (update: unknown, changed = false) => ({
  ...binding,
  changed,
  resourceRevision: null,
  functionEditorProjection: null,
  update,
});

beforeEach(() => {
  vi.resetAllMocks();
  clearGraphSyncBaselines();
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
