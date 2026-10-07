import { describe, expect, it } from "vitest";
import {
  resolveWindowDecorations,
  usesCustomTitleBar,
} from "@/features/application/window/windowDecorationPolicy";

describe("windowDecorationPolicy", () => {
  it("maps custom title bar style to frameless windows", () => {
    expect(resolveWindowDecorations("custom")).toBe(false);
    expect(usesCustomTitleBar("custom")).toBe(true);
  });

  it("maps native title bar style to OS decorations", () => {
    expect(resolveWindowDecorations("native")).toBe(true);
    expect(usesCustomTitleBar("native")).toBe(false);
  });
});
