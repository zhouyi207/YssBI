// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ParameterEditorSpecDto } from "@/shared/types/domain/editorProjection";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
import { ProjectColumnsEditor } from "./RelationalParameterEditors";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

describe("ProjectColumnsEditor", () => {
  it("renders the Rust-issued unavailable reason and validation errors", () => {
    const editor: Extract<ParameterEditorSpecDto, { kind: "projectColumns" }> = {
      kind: "projectColumns",
      allowEmpty: false,
      available: false,
      unavailableReason: "Connect DataFrame input",
      options: [],
      value: [],
    };
    act(() =>
      root.render(
        <ProjectColumnsEditor
          editor={editor}
          errors={["Choose at least one column"]}
          onCommit={vi.fn()}
        />,
      ),
    );

    expect(container.textContent).toContain("Connect DataFrame input");
    expect(container.textContent).toContain("Choose at least one column");
    expect(container.querySelector('button[type="submit"]')).toBeNull();
  });
});
