// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { VariableValueEditorModal } from "./VariableValueEditorModal";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) =>
      ({
        "detail.variableValue.errors.invalidJson": "Invalid JSON",
        "common.confirm": "OK",
      })[key] ?? key,
  }),
}));

vi.mock("@/components/ui/scroll-area", () => ({
  ScrollArea: ({ children }: { children: React.ReactNode }) => children,
}));

function setTextareaValue(element: HTMLTextAreaElement, value: string): void {
  act(() => {
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")?.set?.call(
      element,
      value,
    );
    element.dispatchEvent(new Event("input", { bubbles: true }));
    element.dispatchEvent(new Event("change", { bubbles: true }));
  });
}

describe("VariableValueEditorModal validation", () => {
  let host: HTMLDivElement;
  let root: Root;
  const onSave = vi.fn();
  const onClose = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    document.body.innerHTML = "";
  });

  it("shows invalid JSON inside the dialog and describes the textarea", () => {
    act(() =>
      root.render(
        <VariableValueEditorModal
          open
          onClose={onClose}
          dataType={{ kind: "Array", inner: { kind: "Int64" } }}
          dataValue={{ kind: "Array", value: [] }}
          onSave={onSave}
        />,
      ),
    );

    const textarea = document.querySelector("textarea");
    if (!(textarea instanceof HTMLTextAreaElement)) throw new Error("missing JSON textarea");
    setTextareaValue(textarea, "{not json");
    const confirm = [...document.querySelectorAll("button")].find(
      (button) => button.textContent === "OK",
    );
    act(() => confirm?.dispatchEvent(new MouseEvent("click", { bubbles: true })));

    const alert = document.querySelector<HTMLElement>('[role="alert"]');
    expect(alert?.textContent).toContain("Invalid JSON");
    expect(textarea.getAttribute("aria-describedby")).toBe(alert?.id);
    expect(onSave).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });
});
