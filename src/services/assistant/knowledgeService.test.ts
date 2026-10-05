import { expect, it } from "vitest";
import { z } from "zod";
import fixture from "@/tests/fixtures/node-system-contracts/harness-knowledge.json";
import { citationDetailSchema, knowledgeSourceSchema } from "./knowledgeService";

it("parses shared source and citation payloads and rejects malformed management responses", () => {
  expect(z.array(knowledgeSourceSchema).parse(fixture.sources)).toEqual(fixture.sources);
  expect(citationDetailSchema.parse(fixture.citation)).toEqual(fixture.citation);
  expect(citationDetailSchema.parse(fixture.builtin)).toEqual(fixture.builtin);
  expect(
    knowledgeSourceSchema.safeParse({ ...fixture.sources[0], status: "indexed" }).success,
  ).toBe(false);
  expect(
    knowledgeSourceSchema.safeParse({ ...fixture.sources[0], updatedAt: "1000" }).success,
  ).toBe(false);
  expect(citationDetailSchema.safeParse({ text: "Body" }).success).toBe(false);
  expect(
    citationDetailSchema.safeParse({
      ...fixture.citation,
      resource: { kind: "file", id: "docs/Methods.md" },
    }).success,
  ).toBe(false);
});
