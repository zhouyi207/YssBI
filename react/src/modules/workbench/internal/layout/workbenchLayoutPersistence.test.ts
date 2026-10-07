import { describe, expect, it } from "vitest";
import { Model } from "flexlayout-react";
import { DEFAULT_LOGS_LAYOUT } from "./logsLayoutModel";
import { createEmptyWorkbenchLayout } from "./workbenchLayoutDefaults";
import { parsePersistedWorkbenchLayout } from "./workbenchLayoutPersistence";
import { WorkbenchModelOperations } from "./workbenchLayoutOperations";

describe("persisted workbench envelope", () => {
  it("accepts Assistant only in the fixed left Activity group", () => {
    const ops = new WorkbenchModelOperations(Model.fromJson(createEmptyWorkbenchLayout()));
    ops.ensureView({ viewId: "assistant", title: "Assistant" });
    const root = ops.serialize();
    const nested = { logs: DEFAULT_LOGS_LAYOUT };
    expect(parsePersistedWorkbenchLayout({ root, nested })).toMatchObject({
      root: { status: "valid" },
    });
    const left = root.borders!.find((border) => border.location === "left")!;
    const right = root.borders!.find((border) => border.location === "right")!;
    right.children = left.children;
    right.selected = 0;
    left.children = [];
    left.selected = -1;
    expect(parsePersistedWorkbenchLayout({ root, nested })).toMatchObject({
      root: { status: "invalid" },
      logs: { status: "valid" },
    });
  });

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
