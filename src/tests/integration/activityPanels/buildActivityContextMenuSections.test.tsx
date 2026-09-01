import type { TFunction } from "i18next";
import { describe, expect, it, vi } from "vitest";

import { buildDataSidebarContextMenuSections } from "@/modules/data-explorer/internal/ui/activity/buildDataSidebarContextMenuSections";
import { buildProjectSidebarContextMenuSections } from "@/modules/project-explorer/internal/ui/activity/buildProjectSidebarContextMenuSections";

const t = ((key: string) => key) as TFunction;

function projectActions() {
  return {
    openGraph: vi.fn(),
    createGraph: vi.fn(),
    renameGraphItem: vi.fn(),
    deleteGraphItem: vi.fn(),
    duplicateGraphItem: vi.fn(),
    addVariable: vi.fn(),
    renameVariableItem: vi.fn(),
    deleteVariable: vi.fn(),
    promoteVariable: vi.fn(),
    demoteVariable: vi.fn(),
    canDemoteVariable: true,
    openChart: vi.fn(),
    renameChartItem: vi.fn(),
    duplicateChart: vi.fn(),
    deleteChart: vi.fn(),
    addChart: vi.fn(),
    revealInExplorer: vi.fn(),
  };
}

describe("activity context menu sections", () => {
  it("disables demotion with a reason when no graph scope is active", () => {
    const actions = { ...projectActions(), canDemoteVariable: false };
    const sections = buildProjectSidebarContextMenuSections(
      {
        x: 10,
        y: 20,
        target: { type: "variable", id: "variable-1", name: "Counter", isGlobal: true },
      },
      actions,
      t,
    );

    expect(
      sections.flatMap((section) => section.items).find((item) => item.id === "demote-to-local"),
    ).toMatchObject({ disabled: true, title: "sidebar.noActiveGraph" });
  });

  it("offers both variable scopes from the Variables folder", () => {
    const actions = projectActions();
    const sections = buildProjectSidebarContextMenuSections(
      {
        x: 10,
        y: 20,
        target: { type: "variableSection" },
      },
      actions,
      t,
    );
    const items = sections.flatMap((section) => section.items);

    expect(items.map((item) => item.id)).toEqual(["new-local-variable", "new-global-variable"]);

    items.find((item) => item.id === "new-local-variable")?.onClick?.();
    items.find((item) => item.id === "new-global-variable")?.onClick?.();
    expect(actions.addVariable).toHaveBeenNthCalledWith(1, "New Variable", "Int64", false);
    expect(actions.addVariable).toHaveBeenNthCalledWith(2, "New Variable", "Int64", true);
  });

  it("exposes authoritative chart rename with the opaque path and Rust-provided name", () => {
    const actions = projectActions();
    const sections = buildProjectSidebarContextMenuSections(
      {
        x: 10,
        y: 20,
        target: {
          type: "chart",
          chartPath: "charts/Report.yssbi-chart",
          name: "Report",
        },
      },
      actions,
      t,
    );
    const items = sections.flatMap((section) => section.items);

    expect(items.map((item) => item.id)).toEqual([
      "open",
      "reveal-in-explorer",
      "rename",
      "duplicate",
      "delete",
    ]);

    items.find((item) => item.id === "rename")?.onClick?.();
    expect(actions.renameChartItem).toHaveBeenCalledWith("charts/Report.yssbi-chart", "Report");
  });

  it("keeps data actions out of the Project contribution", () => {
    const actions = {
      openDatabase: vi.fn(),
      renameDatabaseItem: vi.fn(),
      deleteDatabaseItem: vi.fn(),
      importData: vi.fn(),
      revealInExplorer: vi.fn(),
    };
    const sections = buildDataSidebarContextMenuSections(
      { x: 10, y: 20, target: { type: "dataSection" } },
      actions,
      t,
    );

    sections[0]?.items[0]?.onClick?.();
    expect(actions.importData).toHaveBeenCalledOnce();
  });
});
