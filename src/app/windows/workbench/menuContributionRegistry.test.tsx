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

function fileActions() {
  return {
    addEvent: vi.fn(),
    addFunction: vi.fn(),
    addChart: vi.fn(),
    openProject: vi.fn(),
    closeProject: vi.fn(),
    saveGraph: vi.fn(),
    saveGraphAs: vi.fn(),
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

  it("saves the active editor document only while a project is available", () => {
    const actions = fileActions();
    const items = buildFileMenuItems(
      translate,
      { projectAvailable: true, editorCommandAuthorized: true },
      actions,
    );

    const save = items.find((item) => item.label === "common.save");
    expect(save).toMatchObject({ shortcut: "Ctrl+S", onClick: actions.saveGraph });
    save?.onClick?.();
    expect(actions.saveGraph).toHaveBeenCalledOnce();
    expect(actions.saveGraphAs).not.toHaveBeenCalled();

    const withoutProject = buildFileMenuItems(
      translate,
      { projectAvailable: false, editorCommandAuthorized: true },
      actions,
    );
    expect(withoutProject.find((item) => item.label === "common.save")).toMatchObject({
      onClick: undefined,
    });
  });

  it("gates Save and split commands but leaves Save As project-governed", () => {
    const actions = fileActions();
    const splitRight = vi.fn();
    const splitDown = vi.fn();

    const fileItems = buildFileMenuItems(
      translate,
      {
        projectAvailable: true,
        editorCommandAuthorized: false,
      },
      actions,
    );
    const windowItems = buildWindowMenuItems(translate, false, {
      splitRight,
      splitDown,
      openLogsWindow: vi.fn(),
    });

    expect(fileItems.find((item) => item.label === "common.save")).toMatchObject({
      onClick: undefined,
    });
    expect(fileItems.find((item) => item.label === "menubar.saveProjectAs")?.onClick).toBe(
      actions.saveGraphAs,
    );
    expect(
      windowItems.find((item) => item.label === "menubar.splitEditorRight")?.onClick,
    ).toBeUndefined();
    expect(
      windowItems.find((item) => item.label === "menubar.splitEditorDown")?.onClick,
    ).toBeUndefined();
  });
});
