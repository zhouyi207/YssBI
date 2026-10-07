import { expect, it } from "vitest";
import { serializeDataValue } from "@/shared/types/domain/dataValue";
import { parseConstantJsonInput, parseConstantValueInput } from "./valueInput";

const numeric = { kind: "Scalar", inner: "Numeric" } as const;

it("returns an input failure for numeric conversion without losing valid literal precision", () => {
  expect(parseConstantValueInput("not a number", numeric)).toEqual({
    ok: false,
    error: "invalidValue",
  });
  expect(parseConstantJsonInput('["not a number"]', { kind: "Array", inner: numeric })).toEqual({
    ok: false,
    error: "invalidValue",
  });
  expect(parseConstantJsonInput('{"nested":[1e400]}', { kind: "Object" })).toEqual({
    ok: false,
    error: "invalidValue",
  });

  const accepted = parseConstantJsonInput('["9223372036854775807", "1.25"]', {
    kind: "Array",
    inner: numeric,
  });
  expect(accepted.ok).toBe(true);
  if (accepted.ok)
    expect(serializeDataValue(accepted.value)).toEqual({
      List: [{ Integer: "9223372036854775807" }, { Decimal: "1.25" }],
    });
});

it("rejects overflowing or structured table cells instead of changing them during JSON serialization", () => {
  expect(parseConstantJsonInput('{"value":[1e400]}', { kind: "DataFrame" })).toEqual({
    ok: false,
    error: "notDataFrameContent",
  });
  expect(
    parseConstantJsonInput('{"value":[{"nested":1}]}', { kind: "DataSeries", inner: numeric }),
  ).toEqual({
    ok: false,
    error: "notDataSeriesContent",
  });
  expect(parseConstantJsonInput('{"left":[1],"right":[]}', { kind: "DataFrame" })).toEqual({
    ok: false,
    error: "notDataFrameContent",
  });
  expect(
    parseConstantJsonInput('{"a":[1],"b":[2]}', { kind: "DataSeries", inner: numeric }),
  ).toEqual({
    ok: false,
    error: "notDataSeriesContent",
  });
  expect(parseConstantJsonInput('{"value":[1,null,true,"text"]}', { kind: "DataFrame" })).toEqual({
    ok: true,
    value: { kind: "DataFrame", value: '{"value":[1,null,true,"text"]}' },
  });
  expect(parseConstantJsonInput("{}", { kind: "DataSeries", inner: numeric })).toEqual({
    ok: true,
    value: { kind: "Null" },
  });
});
