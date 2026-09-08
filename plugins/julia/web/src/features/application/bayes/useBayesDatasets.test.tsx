// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { useBayesDatasets } from "./useBayesDatasets";
const request = vi.hoisted(() => vi.fn());
vi.mock("@/sdk", () => ({ request }));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
it("uses host-projected dataset metadata without a local project authority", async () => {
  const host = document.createElement("div");
  document.body.appendChild(host);
  const root = createRoot(host);
  let model: ReturnType<typeof useBayesDatasets>;
  function Harness() {
    model = useBayesDatasets();
    return null;
  }
  request.mockResolvedValueOnce({
    datasets: [
      { id: "sales", name: "Sales", columns: [{ name: "amount", type: "Int64", nullable: false }] },
    ],
  });
  try {
    await act(async () => root.render(<Harness />));
    expect(request).toHaveBeenCalledWith("data.list");
    expect(model!.datasets).toEqual([
      {
        sourceId: "sales",
        sourceType: "table",
        displayName: "Sales",
        columns: [{ name: "amount", dtype: "integer", nullable: false }],
      },
    ]);
    expect(model!.loading).toBe(false);
  } finally {
    act(() => root.unmount());
    host.remove();
  }
});
