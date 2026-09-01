// @vitest-environment happy-dom

import { beforeEach, describe, expect, it } from "vitest";
import { applySmoothScrollSetting } from "./appearanceRuntime";

describe("appearanceRuntime", () => {
  beforeEach(() => {
    delete document.documentElement.dataset.smoothScroll;
  });

  it("applySmoothScrollSetting toggles html data attribute", () => {
    applySmoothScrollSetting(true);
    expect(document.documentElement.dataset.smoothScroll).toBe("true");
    applySmoothScrollSetting(false);
    expect(document.documentElement.dataset.smoothScroll).toBe("false");
  });
});
