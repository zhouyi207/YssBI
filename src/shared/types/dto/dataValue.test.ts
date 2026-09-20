import { expect, it } from "vitest";
import {
  deserializeDataValue,
  serializeDataValue,
  inferDataValueFromJson,
} from "../domain/dataValue";
import { isRustDataValueWire } from "./dataValue";

it("round trips full-width integers without JavaScript rounding", () => {
  for (const wire of [
    { Integer: "-9223372036854775808" },
    { Integer: "9223372036854775807" },
    { Unsigned: "18446744073709551615" },
  ]) {
    expect(isRustDataValueWire(wire)).toBe(true);
    expect(serializeDataValue(deserializeDataValue(wire))).toEqual(wire);
  }
  for (const wire of [
    { Integer: 7 },
    { Integer: "01" },
    { Integer: "-0" },
    { Integer: "9223372036854775808" },
    { Unsigned: "18446744073709551616" },
  ]) {
    expect(isRustDataValueWire(wire)).toBe(false);
  }
});

it("preserves nested constant objects, arrays and null through the Rust wire", () => {
  const value = inferDataValueFromJson({
    "": null,
    nested: { items: [1, true, null, "text", { x: 1.5 }] },
  });
  const wire = serializeDataValue(value);
  expect(isRustDataValueWire(wire)).toBe(true);
  expect(wire).toEqual({
    Object: {
      "": "Null",
      nested: {
        Object: {
          items: {
            List: [
              { Integer: "1" },
              { Bool: true },
              "Null",
              { String: "text" },
              { Object: { x: { Decimal: "1.5" } } },
            ],
          },
        },
      },
    },
  });
  expect(deserializeDataValue(wire)).toEqual(value);
});
