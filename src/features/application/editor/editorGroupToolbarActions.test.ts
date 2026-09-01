import { describe, expect, it } from "vitest";
import { prepareEditorGroupToolbarActions } from "./editorGroupToolbarActions";

describe("prepareEditorGroupToolbarActions", () => {
  it("active group shows split and close inline", () => {
    expect(
      prepareEditorGroupToolbarActions({
        isGroupActive: true,
        alwaysShowEditorActions: false,
      }),
    ).toEqual({
      primary: ["split-pointer", "close-group"],
      secondary: [],
    });
  });

  it("inactive group moves actions to overflow only", () => {
    expect(
      prepareEditorGroupToolbarActions({
        isGroupActive: false,
        alwaysShowEditorActions: false,
      }),
    ).toEqual({
      primary: [],
      secondary: ["split-right", "split-down", "close-group"],
    });
  });

  it("alwaysShowEditorActions treats inactive like active", () => {
    expect(
      prepareEditorGroupToolbarActions({
        isGroupActive: false,
        alwaysShowEditorActions: true,
      }),
    ).toEqual({
      primary: ["split-pointer", "close-group"],
      secondary: [],
    });
  });
});
