import { describe, expect, it, vi } from "vitest";
import {
  buildEditMenuItems,
  buildFileMenuItems,
  buildWindowMenuItems,
} from "./menuContributionRegistry";

const translate = (key: string) => key;

function menuActions() {
  return {
    undo: vi.fn(),
    redo: vi.fn(),
    cut: vi.fn(),
    copy: vi.fn(),
    paste: vi.fn(),
    deleteSelected: vi.fn(),
  };
}

describe("Menubar editor command authorization", () => {
  it("does not authorize mutations from a stale activeResourceRef", () => {
    const items = buildEditMenuItems(
      translate,
      {
        activeResourceRef: "events/Stale.yssbi-event",
        canUndo: true,
        canRedo: true,
        editorCommandAuthorized: false,
      },
      menuActions(),
    );

    for (const label of [
      "common.undo",
      "common.redo",
      "menubar.cut",
      "menubar.copy",
      "menubar.paste",
      "common.delete",
    ]) {
      expect(items.find((item) => item.label === label)?.onClick).toBeUndefined();
    }
  });

  it("gates Save and split commands but leaves Save As project-governed", () => {
    const saveGraph = vi.fn();
    const saveGraphAs = vi.fn();
    const splitRight = vi.fn();
    const splitDown = vi.fn();

    const fileItems = buildFileMenuItems(
      translate,
      {
        projectAvailable: true,
        editorCommandAuthorized: false,
      },
      {
        addEvent: vi.fn(),
        addFunction: vi.fn(),
        openProject: vi.fn(),
        closeProject: vi.fn(),
        saveGraph,
        saveGraphAs,
      },
    );
    const windowItems = buildWindowMenuItems(translate, false, {
      splitRight,
      splitDown,
      openLogsWindow: vi.fn(),
    });

    expect(fileItems.find((item) => item.label === "menubar.saveProject")?.onClick).toBeUndefined();
    expect(fileItems.find((item) => item.label === "menubar.saveProjectAs")?.onClick).toBe(
      saveGraphAs,
    );
    expect(
      windowItems.find((item) => item.label === "menubar.splitEditorRight")?.onClick,
    ).toBeUndefined();
    expect(
      windowItems.find((item) => item.label === "menubar.splitEditorDown")?.onClick,
    ).toBeUndefined();
  });
});
