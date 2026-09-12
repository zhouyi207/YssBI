import { invoke } from "@tauri-apps/api/core";
import { expect, it, vi } from "vitest";
import type { GraphDraftSaveDto } from "@/shared/types/dto/editorMutation";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";
import { GraphDraftService } from "./graphDraftService";

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
  const { document } = makeGraphEditorSession(projection);
  const saved: GraphDraftSaveDto = {
    projectInstanceId,
    resourceRevision: 3,
    document,
    projectionReplacement: { graphPath, projection },
  };
  vi.mocked(invoke)
    .mockResolvedValueOnce(saved)
    .mockResolvedValueOnce({
      ...saved,
      projectInstanceId: "00000000-0000-0000-0000-000000000602",
    });
  const save = () =>
    GraphDraftService.save(projectInstanceId, graphPath, "zh-CN", operationId, document);

  await expect(save()).resolves.toEqual(saved);
  expect(invoke).toHaveBeenCalledWith("save_project_graph", {
    projectInstanceId,
    graphPath,
    locale: "zh-CN",
    operationId,
    document,
  });
  await expect(save()).rejects.toThrow("Graph draft save result is malformed");
});
