import { expect, it } from "vitest";
import { deserializeDataValue, serializeDataValue, type DataValue } from "../domain/dataValue";
import { isRustDataValueWire } from "./dataValue";

it("preserves nested constant objects, arrays and null through the Rust wire", () => {
  const value: DataValue = {
    kind: "Object",
    value: { "": null, nested: { items: [1, true, null, "text", { x: 1.5 }] } },
  };
  const wire = serializeDataValue(value);
  expect(isRustDataValueWire(wire)).toBe(true);
  expect(wire).toEqual({
    Object: {
      "": "Null",
      nested: {
        Object: {
          items: {
            Array: [
              { Int64: 1 },
              { Boolean: true },
              "Null",
              { String: "text" },
              { Object: { x: { Float64: 1.5 } } },
            ],
          },
        },
      },
    },
  });
  expect(deserializeDataValue(wire)).toEqual(value);
});
