// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { applyGraphDraftMutation } from "@/features/application/graphDraft/graphDraftCoordinator";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";
import { NodeConfigurationPanel } from "./NodeConfigurationPanel";

vi.mock("@/features/application/graphDraft/graphDraftCoordinator", () => ({
  applyGraphDraftMutation: vi.fn(),
}));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { resolvedLanguage: "en-US" } }),
}));
(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

afterEach(() => {
  useGraphProjectionStore.setState({ graphEntities: {} });
  vi.clearAllMocks();
});

it("edits configuration on its node without a source selector or configuration pins", async () => {
  const graphPath = "events/configuration.yssbi-event";
  const target = makeEditorProjectionFixture({ graphPath, nodeId: "ols" });
  const projection = target.projection;
  const fields = [
    {
      key: "covariance",
      display: { title: "Covariance estimator", description: null },
      editor: "select" as const,
      presentation: "detailPanel" as const,
      valueType: { kind: "String" as const },
      multiline: false,
      value: "nonrobust",
      configuration: { kind: "selectOptions" as const, options: ["nonrobust", "HC1"] },
    },
  ];
  projection.nodes[0]!.parameterEditors = [
    {
      key: "configuration",
      display: { title: "Model configuration", description: null },
      editor: "configuration",
      presentation: "detailPanel",
      valueType: { kind: "Object" },
      multiline: false,
      value: { covariance: "nonrobust" },
      configuration: { kind: "configuration", fields },
    },
  ];
  projection.nodes[0]!.ports = [];
  projection.connections = [];
  expect(useGraphProjectionStore.getState().replaceProjection(graphPath, projection).applied).toBe(
    true,
  );
  vi.mocked(applyGraphDraftMutation).mockResolvedValue({
    status: "applied",
    result: {} as never,
    insertedNodeIds: [],
  });
  const container = document.createElement("div");
  const root = createRoot(container);
  try {
    await act(async () =>
      root.render(<NodeConfigurationPanel graphPath={graphPath} nodeId="ols" />),
    );
    const selector = container.querySelector<HTMLSelectElement>(
      'select[aria-label="Covariance estimator"]',
    )!;
    expect(selector.value).toBe("nonrobust");
    await act(async () => {
      selector.value = "HC1";
      selector.dispatchEvent(new Event("change", { bubbles: true }));
    });
    expect(applyGraphDraftMutation).toHaveBeenLastCalledWith({
      graphPath,
      locale: "en-US",
      mutation: {
        type: "setConfiguration",
        payload: { nodeId: "ols", key: "configuration", values: { covariance: "HC1" } },
      },
    });

    expect(container.querySelectorAll("select")).toHaveLength(1);
    expect(container.textContent).toContain("Model configuration");
    fields[0]!.value = "HC1";
    await act(async () => {
      expect(
        useGraphProjectionStore.getState().replaceProjection(graphPath, projection).applied,
      ).toBe(true);
    });
    expect(selector.value).toBe("HC1");
  } finally {
    await act(async () => root.unmount());
  }
});
