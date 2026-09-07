import { toGraphResourceUri } from "@/shared/types/domain/graphResourcePath";
import { describe, expect, it } from "vitest";
import type { ProjectResourceMeta } from "./resourceTypes";
import { lookupGraphResource } from "./resourceQueries";

describe("resourceQueries", () => {
  it("looks up graph resources by their opaque path", () => {
    const event = {
      id: "opaque graph A",
      kind: "event",
      name: "Main",
      uri: toGraphResourceUri("event", "opaque graph A"),
      exists: true,
      loaded: false,
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    } satisfies ProjectResourceMeta;
    const functionResource = {
      id: "events/still-a-function",
      kind: "function",
      name: "Helper",
      uri: toGraphResourceUri("function", "events/still-a-function"),
      exists: true,
      loaded: false,
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    } satisfies ProjectResourceMeta;
    const resources = {
      [event.uri]: event,
      [functionResource.uri]: functionResource,
    };

    expect(lookupGraphResource(resources, event.id)).toBe(event);
    expect(lookupGraphResource(resources, functionResource.id)).toBe(functionResource);
    expect(lookupGraphResource(resources, "functions/Missing.yssbi-function")).toBeUndefined();
  });
});
