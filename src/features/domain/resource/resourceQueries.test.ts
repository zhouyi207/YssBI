import { describe, expect, it } from "vitest";
import type { ProjectResourceMeta } from "./resourceTypes";
import { lookupGraphResource } from "./resourceQueries";

describe("resourceQueries", () => {
  it("looks up graph resources by their opaque path", () => {
    const event = {
      id: "events/Main.yssbi-event",
      kind: "event",
      name: "Main",
      uri: "yssbi://graph/event/events::Main.yssbi-event",
      exists: true,
      loaded: false,
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    } satisfies ProjectResourceMeta;
    const functionResource = {
      id: "functions/Helper.yssbi-function",
      kind: "function",
      name: "Helper",
      uri: "yssbi://graph/function/functions::Helper.yssbi-function",
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
