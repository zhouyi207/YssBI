import { expect, it } from "vitest";
import { toResourceUri } from "./resource";

it("keeps opaque resource identities distinct in frontend store keys", () => {
  const paths = ["opaque graph", "events/a", "events//a", "eventGraphs::a", "a%2Fb", "a/b"];
  const keys = paths.map((path) => toResourceUri("function_graph", path));
  expect(new Set(keys).size).toBe(paths.length);
  keys.forEach((key, index) => {
    expect(decodeURIComponent(key.slice(key.lastIndexOf("/") + 1))).toBe(paths[index]);
  });
  expect(toResourceUri("event_graph", paths[0])).not.toBe(keys[0]);
});
