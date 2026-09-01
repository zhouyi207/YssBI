import { describe, expect, it } from "vitest";
import type { SidebarPanelModel } from "@/features/core/sidebar";
import { flattenSidebarPanelModel } from "./sidebarRenderRows";

const emptyData: SidebarPanelModel = {
  sections: [
    {
      key: "dataData",
      label: "Data",
      expanded: true,
      rows: [],
      emptyMessage: "No data",
    },
  ],
};

describe("flattenSidebarPanelModel", () => {
  it("emits section empty rows only for expanded empty sections", () => {
    expect(flattenSidebarPanelModel(emptyData)).toEqual([
      {
        kind: "section",
        rowKey: "section:dataData",
        sectionKey: "dataData",
        level: 0,
        label: "Data",
        expanded: true,
      },
      {
        kind: "sectionEmpty",
        rowKey: "section-empty:dataData",
        sectionKey: "dataData",
        level: 1,
        message: "No data",
      },
    ]);
  });

  it("places populated rows after their expanded section header", () => {
    const model: SidebarPanelModel = {
      sections: [
        {
          key: "dataData",
          label: "Data",
          expanded: true,
          emptyMessage: "No data",
          rows: [
            {
              kind: "database",
              rowKey: "database:db-1",
              level: 1,
              id: "db-1",
              name: "Sales",
              data: { name: "Sales" },
            },
          ],
        },
      ],
    };

    expect(flattenSidebarPanelModel(model).map((row) => row.kind)).toEqual(["section", "database"]);
  });

  it("does not synthesize a placeholder without an empty message", () => {
    const model: SidebarPanelModel = {
      sections: [{ key: "dataData", label: "Data", expanded: true, rows: [] }],
    };
    expect(flattenSidebarPanelModel(model)).toHaveLength(1);
  });
});
