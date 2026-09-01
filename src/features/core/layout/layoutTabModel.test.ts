import { describe, expect, it } from "vitest";

import { buildGraphLayoutTab, buildWorksheetLayoutTab } from "./layoutTabModel";

describe("layoutTabModel", () => {
  it("buildGraphLayoutTab and buildWorksheetLayoutTab produce typed tabs", () => {
    expect(buildGraphLayoutTab("events/Main.yssbi-event", "event")).toMatchObject({
      id: "events/Main.yssbi-event",
      type: "event",
      component: "GraphEditor",
    });
    expect(buildGraphLayoutTab("functions/Helper.yssbi-function", "function")).toMatchObject({
      id: "functions/Helper.yssbi-function",
      type: "function",
    });
    const worksheetPath = "worksheets/Opaque Path With Spaces.yssbi-worksheet";
    expect(buildWorksheetLayoutTab(worksheetPath)).toMatchObject({
      id: worksheetPath,
      type: "worksheet",
      component: "WorksheetEditor",
    });
  });
});
