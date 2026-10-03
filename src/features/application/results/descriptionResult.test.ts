import { expect, it } from "vitest";
import { descriptionColumns } from "./descriptionResult";
import { parseReportPayloadResult } from "@/shared/types/report/parseReportPayload";

it("reads type-specific summaries and complete category frequencies without losing codes or order", () => {
  const categorical = {
    position: 1,
    semantic: "Categorical",
    count: "9007199254740993",
    missing: 0,
    unique: 2,
    categories: {
      "2": { value: "__proto__", frequency: 1, proportion: 1.1102230246251565e-16 },
      "1": {
        value: "",
        label: "空文本",
        frequency: "9007199254740992",
        proportion: 0.9999999999999999,
      },
    },
  };
  const numeric = {
    position: 2,
    semantic: "Numeric",
    count: 1,
    missing: 2,
    mean: 5,
    std: null,
    min: 5,
    q25: 5,
    median: 5,
    q75: 5,
    max: 5,
  };
  const parsedCategory = {
    ...categorical,
    categories: [
      { position: 1, ...categorical.categories["1"] },
      { position: 2, ...categorical.categories["2"] },
    ],
  };
  const result = { columns: { " amount/~ ": numeric, 分类: categorical } };
  expect(descriptionColumns(result)).toEqual([
    { column: "分类", ...parsedCategory },
    { column: " amount/~ ", ...numeric },
  ]);
  expect(parseReportPayloadResult("structured", result)).toEqual({
    ok: true,
    value: { kind: "structured", data: result, sections: {} },
  });
  expect(descriptionColumns({ columns: { ["__proto__"]: categorical } })).toEqual([
    { column: "__proto__", ...parsedCategory },
  ]);
  for (const semantic of ["Ordinal", "Binary"]) {
    expect(descriptionColumns({ columns: { level: { ...categorical, semantic } } })).toEqual([
      { column: "level", ...parsedCategory, semantic },
    ]);
  }
  const empty = {
    ...categorical,
    count: 0,
    missing: 3,
    unique: 0,
    categories: {},
  };
  expect(descriptionColumns({ columns: { empty } })).toEqual([
    { column: "empty", ...empty, categories: [] },
  ]);
  const { missing: _, ...incomplete } = categorical;
  for (const invalid of [
    incomplete,
    { ...categorical, count: -1 },
    { ...categorical, mean: null },
    { ...categorical, mode: "" },
    { ...numeric, mode: null },
    { ...numeric, mean: Infinity },
    { ...categorical, categories: { "2": categorical.categories["2"] } },
    { ...categorical, categories: { "1": { ...categorical.categories["1"], proportion: 2 } } },
    { ...categorical, categories: [] },
    null,
  ]) {
    expect(descriptionColumns({ columns: { valid: numeric, invalid } })).toBeNull();
  }
  expect(
    descriptionColumns({ columns: { a: categorical, b: { ...numeric, position: 1 } } }),
  ).toBeNull();
  expect(descriptionColumns({ columns: [] })).toBeNull();
});
