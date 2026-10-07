import { expect, it } from "vitest";
import { validateResourceMutationResult } from "./resourceMutationValidation";
import { parseResourceMutationResultDto } from "@/shared/types/dto/resourceMutationResultWireParser";
import { areResourceDeltasValid } from "@/shared/types/dto/resourceMutationWireValidator";
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

it("validates and isolates current resource payloads with exact signature and patch fields", () => {
  const operationId = "00000000-0000-0000-0000-000000000123";
  const path = "functions/Compute.yssbi-function";
  const signature = {
    parameters: [{ id: "opaque-id", name: "Input 名称", type_name: "Future<Opaque Rust Type>" }],
    return_type: null,
  };
  const patch = { before: { parameters: [], return_type: null }, after: signature };
  const result: ResourceMutationResultDto = {
    operationId,
    projectInstanceId: "project-a",
    publicationRevision: 2,
    moves: [],
    projectionReplacements: [],
    projectionStatus: { status: "incomplete", invalidatedGraphPaths: [path] },
    deltas: [
      {
        resource: { kind: "function", key: path },
        fromRevision: 1,
        toRevision: 2,
        causedBy: operationId,
        payload: { kind: "function", patch },
      },
    ],
  };
  const parsed = parseResourceMutationResultDto(result);
  expect(parsed).toEqual(result);
  const unsupportedGraphResult = {
    ...result,
    deltas: [
      {
        ...result.deltas[0],
        resource: { kind: "graph", key: "events/Main.yssbi-event" },
        payload: { kind: "graph", patch: { operations: [] } },
      },
    ],
  };
  expect.soft(areResourceDeltasValid(unsupportedGraphResult.deltas)).toBe(false);
  expect
    .soft(() => parseResourceMutationResultDto(unsupportedGraphResult))
    .toThrow("resource deltas are malformed");
  expect
    .soft(
      validateResourceMutationResult(
        unsupportedGraphResult as unknown as ResourceMutationResultDto,
      ),
    )
    .toBe("resource deltas are malformed");
  for (const invalidPatch of [
    { ...patch, extra: true },
    { ...patch, after: { ...signature, extra: true } },
    {
      ...patch,
      after: { ...signature, parameters: [{ ...signature.parameters[0], extra: true }] },
    },
    { ...patch, after: { ...signature, return_type: 42 } },
  ]) {
    expect(() =>
      parseResourceMutationResultDto({
        ...result,
        deltas: [{ ...result.deltas[0], payload: { kind: "function", patch: invalidPatch } }],
      }),
    ).toThrow("resource deltas are malformed");
  }
  signature.parameters[0].name = "External mutation";
  const parsedPayload = parsed.deltas[0].payload;
  if (parsedPayload.kind !== "function") throw new Error("Expected function delta");
  expect(parsedPayload.patch.after.parameters[0].name).toBe("Input 名称");
});
