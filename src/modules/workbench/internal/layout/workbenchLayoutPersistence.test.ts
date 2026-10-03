import { describe, expect, it } from "vitest";
import { DEFAULT_LOGS_LAYOUT } from "./logsLayoutModel";
import { createEmptyWorkbenchLayout } from "./workbenchLayoutDefaults";
import { parsePersistedWorkbenchLayout } from "./workbenchLayoutPersistence";

describe("persisted workbench envelope", () => {
  it("rejects unknown envelope fields while validating root and Logs independently", () => {
    const root = createEmptyWorkbenchLayout();
    const nested = { logs: DEFAULT_LOGS_LAYOUT };
    expect(parsePersistedWorkbenchLayout({ root, nested })).toMatchObject({
      root: { status: "valid" },
      logs: { status: "valid" },
    });
    expect(parsePersistedWorkbenchLayout({ root, nested, extra: true })).toBeNull();
    expect(parsePersistedWorkbenchLayout({ root, nested: { ...nested, extra: true } })).toBeNull();
    expect(parsePersistedWorkbenchLayout({ root, nested: {} })).toMatchObject({
      root: { status: "valid" },
      logs: { status: "invalid" },
    });
    expect(parsePersistedWorkbenchLayout({ nested })).toMatchObject({
      root: { status: "invalid" },
      logs: { status: "valid" },
    });
  });
});
