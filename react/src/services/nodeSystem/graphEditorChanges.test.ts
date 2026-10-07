import { expect, it } from "vitest";
import fixture from "@/tests/fixtures/node-system-contracts/graph-projection-splice.json";
import { applyGraphChanges, graphChangesSchema } from "./graphEditorChanges";

it("installs the Rust splice wire without replacing unaffected or shifted records", () => {
  const before = structuredClone(fixture.before);
  const result = applyGraphChanges(
    before,
    graphChangesSchema.parse(fixture.changes),
  ) as typeof fixture.after;
  expect(result).toEqual(fixture.after);
  expect(before).toEqual(fixture.before);
  expect(result.unchanged).toBe(before.unchanged);
  expect(result.parameters[0]).toBe(before.parameters[0]);
  expect(result.parameters[2]).toBe(before.parameters[1]);
  expect(result.parameters[3]).toBe(before.parameters[2]);
});

it("applies a batch atomically and never mutates inserted wire values on later operations", () => {
  const before = { stable: { value: 1 }, items: [{ value: 2 }] };
  const inserted = { value: 3 };
  const changes = graphChangesSchema.parse([
    { kind: "set", path: ["new"], value: inserted },
    { kind: "set", path: ["new", "value"], value: 4 },
    { kind: "splice", path: ["items"], index: 1, deleteCount: 0, values: [inserted] },
    { kind: "set", path: ["items", "1", "value"], value: 5 },
  ]);
  const next = applyGraphChanges(before, changes) as typeof before & { new: { value: number } };
  expect(next.new.value).toBe(4);
  expect(next.items[1].value).toBe(5);
  expect(next.items[0]).toBe(before.items[0]);
  expect(inserted.value).toBe(3);
  for (const invalid of [
    { kind: "remove", path: ["absent"] },
    { kind: "set", path: ["__proto__", "polluted"], value: true },
    { kind: "splice", path: ["items"], index: 0, deleteCount: 3, values: [] },
  ]) {
    expect(() =>
      applyGraphChanges(before, graphChangesSchema.parse([...changes, invalid])),
    ).toThrow();
    expect(before).toEqual({ stable: { value: 1 }, items: [{ value: 2 }] });
    expect(inserted.value).toBe(3);
  }
  for (const invalid of [
    [{ kind: "splice", path: [], index: -1, deleteCount: 0, values: [] }],
    [{ kind: "splice", path: [], index: 0, deleteCount: 513, values: [] }],
    [{ kind: "set", path: [], value: {}, unexpected: true }],
    [{ kind: "set", path: [] }],
  ])
    expect(graphChangesSchema.safeParse(invalid).success).toBe(false);
});
