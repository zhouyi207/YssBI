// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SqlConnectionModal } from "./SqlConnectionModal";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) =>
      ({
        "importModal.connectTo": "Connect PostgreSQL",
        "importModal.close": "Close",
        "importModal.formConfig": "Form",
        "importModal.connectionString": "Connection string",
        "importModal.host": "Host",
        "importModal.port": "Port",
        "importModal.username": "Username",
        "importModal.password": "Password",
        "importModal.database": "Database",
        "importModal.connect": "Connect",
        "importModal.connectionFieldsInvalid": "Invalid connection fields",
        "common.cancel": "Cancel",
      })[key] ?? key,
  }),
}));

function setInputValue(element: HTMLInputElement, value: string): void {
  act(() => {
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(element, value);
    element.dispatchEvent(new Event("input", { bubbles: true }));
    element.dispatchEvent(new Event("change", { bubbles: true }));
  });
}

describe("SqlConnectionModal validation", () => {
  let host: HTMLDivElement;
  let root: Root;
  const onConnect = vi.fn();
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

  it("replaces URI encoder prose with localized field feedback", () => {
    act(() =>
      root.render(
        <SqlConnectionModal options={{ engine: "postgres", onConnect }} onClose={onClose} />,
      ),
    );

    const inputs = [...document.querySelectorAll("input")];
    const username = inputs[2];
    if (!(username instanceof HTMLInputElement)) throw new Error("missing username input");
    setInputValue(username, "\uD800");

    const form = document.querySelector("form");
    act(() => form?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })));

    expect(document.body.textContent).toContain("Invalid connection fields");
    expect(document.body.textContent).not.toContain("URI malformed");
    expect(onConnect).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });
});
