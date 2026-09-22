import { afterEach, expect, it } from "vitest";
import { resourceKey, useResourceStore } from "@/features/core/resource";
import { resolveResourceDisplayName } from "./resolveResourceDisplayName";

afterEach(() => useResourceStore.getState().clear());

it("uses the published chart label and falls back to a supplied label or resource path", () => {
  const ref = { id: "charts/opaque.yssbi-chart", kind: "chart" as const };
  useResourceStore.getState().upsertResource({
    ...ref,
    uri: resourceKey(ref),
    name: "Published chart name",
    exists: true,
    loaded: false,
    hasDirtyDocument: false,
    hasStaleDocument: false,
    hasConflictDocument: false,
  });
  expect(resolveResourceDisplayName(ref, "Old name")).toBe("Published chart name");
  useResourceStore.getState().removeResource(ref);
  expect(resolveResourceDisplayName(ref, "Panel title")).toBe("Panel title");
  expect(resolveResourceDisplayName(ref)).toBe(ref.id);
});
