// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import { DataDetailPanel } from "./DataDetailPanel";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const dataframe: DatabaseRecord = {
  id: "sales",
  name: "Sales",
  columns: [
    { name: "amount", type: "Float64" },
    { name: "region", type: "String" },
  ],
  rowCount: 12,
  columnCount: 2,
};

describe("DataDetailPanel", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeAll(() => {
    (
      globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }
    ).IS_REACT_ACT_ENVIRONMENT = true;
  });

  afterAll(() => {
    (
      globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }
    ).IS_REACT_ACT_ENVIRONMENT = false;
  });

  beforeEach(() => {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("reveals column metadata from a collapsible section without a nested scroll area", () => {
    act(() => root.render(<DataDetailPanel dataframe={dataframe} />));

    const columnsTrigger = host.querySelector<HTMLButtonElement>("button[aria-expanded]");
    expect(columnsTrigger?.textContent).toContain("detail.fields.columns");
    expect(columnsTrigger?.getAttribute("aria-expanded")).toBe("false");
    expect(host.textContent).not.toContain("amount");
    expect(host.querySelectorAll('[data-slot="scroll-area"]')).toHaveLength(1);

    act(() => columnsTrigger?.dispatchEvent(new MouseEvent("click", { bubbles: true })));

    expect(columnsTrigger?.getAttribute("aria-expanded")).toBe("true");
    expect(host.textContent).toContain("amount");
    expect(host.textContent).toContain("Float64");
  });
});
