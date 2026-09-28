import type { TFunction } from "i18next";
import { describe, expect, it, vi } from "vitest";

import { buildProjectSidebarContextMenuSections } from "@/modules/project-explorer/internal/ui/activity/buildProjectSidebarContextMenuSections";

const t = ((key: string) => key) as TFunction;

function projectActions() {
  return {
    createFile: vi.fn(),
    openFile: vi.fn(),
    renameFile: vi.fn(),
    duplicateFile: vi.fn(),
    deleteFile: vi.fn(),
    openDatabase: vi.fn(),
    renameDatabaseItem: vi.fn(),
    deleteDatabaseItem: vi.fn(),
    importData: vi.fn(),
    revealInExplorer: vi.fn(),
  };
}

describe("activity context menu sections", () => {
  it("exposes authoritative chart rename with the opaque path and Rust-provided name", () => {
    const actions = projectActions();
    const sections = buildProjectSidebarContextMenuSections(
      {
        x: 10,
        y: 20,
        target: {
          type: "file",
          kind: "chart",
          id: "charts/Report.yssbi-chart",
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
    expect(actions.renameFile).toHaveBeenCalledWith(
      expect.objectContaining({ id: "charts/Report.yssbi-chart", kind: "chart" }),
      "Report",
    );
  });

  it("routes data import and resource management through the Project contribution", () => {
    const actions = projectActions();
    const sections = buildProjectSidebarContextMenuSections(
      { x: 10, y: 20, target: { type: "dataSection" } },
      actions,
      t,
    );

    sections[0]?.items[0]?.onClick?.();
    expect(actions.importData).toHaveBeenCalledOnce();

    const items = buildProjectSidebarContextMenuSections(
      { x: 10, y: 20, target: { type: "database", id: "database-id", name: "Sales" } },
      actions,
      t,
    ).flatMap((section) => section.items);
    for (const id of ["open", "reveal-in-explorer", "rename", "delete"]) {
      items.find((item) => item.id === id)?.onClick?.();
    }
    expect(actions.openDatabase).toHaveBeenCalledWith("database-id");
    expect(actions.revealInExplorer).toHaveBeenCalledWith({
      kind: "database",
      resourceId: "database-id",
    });
    expect(actions.renameDatabaseItem).toHaveBeenCalledWith("database-id", "Sales");
    expect(actions.deleteDatabaseItem).toHaveBeenCalledWith("database-id", "Sales");
  });
});
