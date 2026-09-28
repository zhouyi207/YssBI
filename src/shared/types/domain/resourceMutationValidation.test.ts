import { expect, it } from "vitest";
import { validateResourceMutationResult } from "./resourceMutationValidation";
import { parseResourceMutationResultDto } from "@/shared/types/dto/resourceMutationResultWireParser";
import type { ResourceMutationResultDto } from "./editorMutation";

it("enforces the same deletion revision transition at both receipt boundaries", () => {
  const operationId = "00000000-0000-0000-0000-000000000123";
  const result: ResourceMutationResultDto = {
    projectInstanceId: "project-a",
    operationId,
    publicationRevision: 1,
    moves: [],
    projectionReplacements: [],
    projectionStatus: { status: "complete", expectedGraphPaths: [] },
    deltas: [
      {
        resource: { kind: "doc", key: "docs/Report.md" },
        fromRevision: 0,
        toRevision: 1,
        causedBy: operationId,
        payload: {
          kind: "resource_lifecycle",
          patch: {
            before: { path: "docs/Report.md", kind: "doc", name: "Report", revision: 0 },
            after: null,
          },
        },
      },
    ],
  };
  expect(validateResourceMutationResult(result)).toBeUndefined();
  expect(parseResourceMutationResultDto(result)).toEqual(result);
  result.deltas[0].toRevision = 2;
  expect(validateResourceMutationResult(result)).toBe("resource deltas are malformed");
  expect(() => parseResourceMutationResultDto(result)).toThrow();
});
