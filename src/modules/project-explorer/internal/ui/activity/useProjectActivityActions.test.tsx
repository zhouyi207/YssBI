// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useProjectActivityActions } from "./useProjectActivityActions";

const mocks = vi.hoisted(() => ({
  addVariable: vi.fn(),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock("@/features/application/editor", () => ({
  useEditorPanelCommands: () => ({ openGraph: vi.fn() }),
  useOpenChart: () => vi.fn(),
  useChartManagement: () => ({}),
}));

vi.mock("@/features/application/dataManagement", () => ({
  useGraphManagement: () => ({}),
  useVariableManagement: () => ({
    addVariable: mocks.addVariable,
  }),
}));

vi.mock("@/features/application/sidebar", () => ({
  useActiveProjectGraph: () => null,
}));

vi.mock("@/features/core/variable/read", () => ({
  useVariableRead: (selector: (state: { variables: Record<string, never> }) => unknown) =>
    selector({ variables: {} }),
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("useProjectActivityActions", () => {
  let root: Root;
  let host: HTMLDivElement;
  let actions: ReturnType<typeof useProjectActivityActions>;

  beforeEach(() => {
    vi.clearAllMocks();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    function Harness() {
      actions = useProjectActivityActions(vi.fn());
      return null;
    }
    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("does not expose local Project actions for a chart active panel", async () => {
    expect(actions.canDemoteVariable).toBe(false);

    await act(async () => {
      await actions.addVariable("Threshold", "Int64", false);
    });

    expect(mocks.addVariable).not.toHaveBeenCalled();
  });
});
