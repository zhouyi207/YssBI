import { toResourceUri } from "@/shared/types/domain/resource";
import { describe, expect, it } from "vitest";
import type { ProjectResourceMeta } from "./resourceTypes";
import { lookupNodeFileResource } from "./resourceQueries";

describe("resourceQueries", () => {
  it("looks up graph resources by their opaque path", () => {
    const event = {
      id: "opaque graph A",
      kind: "event_graph",
      name: "Main",
      uri: toResourceUri("event_graph", "opaque graph A"),
      exists: true,
      loaded: false,
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    } satisfies ProjectResourceMeta;
    const functionResource = {
      id: "events/still-a-function",
      kind: "function_graph",
      name: "Helper",
      uri: toResourceUri("function_graph", "events/still-a-function"),
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

    expect(lookupNodeFileResource(resources, event.id)).toBe(event);
    expect(lookupNodeFileResource(resources, functionResource.id)).toBe(functionResource);
    expect(lookupNodeFileResource(resources, "functions/Missing.yssbi-function")).toBeUndefined();
  });
});
