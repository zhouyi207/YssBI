import { expect, it } from "vitest";
import { toGraphResourceUri } from "./graphResourcePath";

it("keeps opaque resource identities distinct in frontend store keys", () => {
  const paths = ["opaque graph", "events/a", "events//a", "events::a", "a%2Fb", "a/b"];
  const keys = paths.map((path) => toGraphResourceUri("function", path));
  expect(new Set(keys).size).toBe(paths.length);
  keys.forEach((key, index) => {
    expect(decodeURIComponent(key.slice(key.lastIndexOf("/") + 1))).toBe(paths[index]);
  });
  expect(toGraphResourceUri("event", paths[0])).not.toBe(keys[0]);
});
