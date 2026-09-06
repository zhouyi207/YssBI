import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ResourceMutationResultDto } from "@/shared/types/dto/editorMutation";
import { VariableService } from "./variableService";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const variableId = "00000000-0000-0000-0000-000000000701";
const mutation: ResourceMutationResultDto = {
  operationId: "00000000-0000-0000-0000-000000000702",
  projectInstanceId: "00000000-0000-0000-0000-000000000703",
  publicationRevision: 2,
  moves: [],
  deltas: [],
  projectionReplacements: [],
  projectionStatus: { status: "complete", expectedGraphPaths: [] },
};

describe("VariableService mutation receipts", () => {
  beforeEach(() => vi.clearAllMocks());

  it("sends a revisioned rename and returns the authoritative publication", async () => {
    const receipt = { variableId, mutation };
    vi.mocked(invoke).mockResolvedValue(receipt);

    await expect(
      VariableService.updateVariable(
        mutation.projectInstanceId,
        mutation.operationId,
        1,
        variableId,
        { name: "Renamed" },
      ),
    ).resolves.toEqual(receipt);
    expect(invoke).toHaveBeenCalledWith("update_variable", {
      projectInstanceId: mutation.projectInstanceId,
      operationId: mutation.operationId,
      expectedRevision: 1,
      variableId,
      name: "Renamed",
      dataType: null,
      dataValue: null,
      description: null,
      tags: null,
    });
  });

  it("rejects an absent or malformed publication before local state can be updated", async () => {
    for (const response of [
      { variableId, mutation: null },
      { variableId, mutation: { ...mutation, publicationRevision: -1 } },
      { variableId, variable: null, result: null },
    ]) {
      vi.mocked(invoke).mockResolvedValue(response);
      await expect(
        VariableService.deleteVariable(
          mutation.projectInstanceId,
          mutation.operationId,
          1,
          variableId,
        ),
      ).rejects.toThrow();
    }
  });
});
