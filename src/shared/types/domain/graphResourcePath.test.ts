import { describe, expect, it } from "vitest";
import {
  decodeGraphResourceKey,
  encodeGraphResourceKey,
  parseGraphResourceUri,
  inferGraphResourceKind,
  isValidGraphResourceTabId,
  toGraphResourceUri,
} from "./graphResourcePath";

describe("graphResourcePath", () => {
  it("round-trips nested paths through encode/decode", () => {
    expect(encodeGraphResourceKey("functions/math/add")).toBe("functions::math::add");
    expect(decodeGraphResourceKey("functions::math::add")).toBe("functions/math/add");
  });

  it("round-trips graph resource URIs", () => {
    const uri = toGraphResourceUri("function", "functions/My Fn");
    expect(uri).toBe("yssbi://graph/function/functions::My Fn");
    expect(parseGraphResourceUri(uri)).toEqual({
      kind: "function",
      path: "functions/My Fn",
    });
  });

  it("rejects non-graph URIs", () => {
    expect(parseGraphResourceUri("file:///tmp/x")).toBeNull();
  });

  it("infers graph kind from persisted paths", () => {
    expect(inferGraphResourceKind("events/Main.yssbi-event")).toBe("event");
    expect(inferGraphResourceKind("functions/Helper.yssbi-function")).toBe("function");
    expect(isValidGraphResourceTabId("events/Main.yssbi-event", "event")).toBe(true);
    expect(isValidGraphResourceTabId("events/Main.yssbi-event", "function")).toBe(false);
  });
});
