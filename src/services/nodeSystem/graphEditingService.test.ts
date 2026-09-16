import { invoke } from "@tauri-apps/api/core";
import { expect, it, vi } from "vitest";
import type { GraphSaveResultDto } from "@/shared/types/dto/editorMutation";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";
import { GraphEditingService } from "./graphEditingService";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

it("parses a graph save receipt and rejects a response from another project", async () => {
  const projectInstanceId = "00000000-0000-0000-0000-000000000601";
  const operationId = "00000000-0000-0000-0000-000000000501";
  const graphPath = "events/Main.yssbi-event";
  const { projection } = makeEditorProjectionFixture({
    graphPath,
    nodeId: "00000000-0000-0000-0000-000000000001",
    connectionId: "00000000-0000-0000-0000-000000000002",
  });
  const { document, editing } = makeGraphEditorSession(projection);
  const saved: GraphSaveResultDto = {
    editing,
    projectInstanceId,
    resourceRevision: 3,
    document,
    projectionReplacement: { graphPath, projection },
  };
  const wire = {
    projectInstanceId,
    graphPath,
    locale: "zh-CN",
    changed: false,
    resourceRevision: 3,
    functionEditorProjection: null,
    update: {
      kind: "snapshot",
      cursor: "saved",
      snapshotBytes: JSON.stringify({ document, projection, editing }).length,
      data: { document, projection, editing },
    },
  };
  vi.mocked(invoke)
    .mockResolvedValueOnce(wire)
    .mockResolvedValueOnce({
      ...wire,
      projectInstanceId: "00000000-0000-0000-0000-000000000602",
    });
  const save = () =>
    GraphEditingService.save(projectInstanceId, graphPath, "zh-CN", operationId, editing.version);

  await expect(save()).resolves.toEqual(saved);
  expect(invoke).toHaveBeenCalledWith("save_project_graph", {
    projectInstanceId,
    graphPath,
    locale: "zh-CN",
    operationId,
    version: editing.version,
    cursor: null,
  });
  await expect(save()).rejects.toThrow("Graph response binding is malformed");
});
