// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import type { BayesModelDraftDTO } from "@/shared/types/bayes";
const mocks = vi.hoisted(() => ({ submit: vi.fn() }));
vi.mock("@/services/bayes", () => ({
  submitBayesInference: mocks.submit,
  getBayesInferenceStatus: vi.fn(),
  readBayesInferenceResult: vi.fn(),
  cancelBayesInference: vi.fn(),
}));
import { useBayesInferenceTask } from "./useBayesInferenceTask";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
it("retries the captured submission without creating a new operation or using edited draft data", async () => {
  mocks.submit
    .mockRejectedValueOnce({ code: "plugin_request_timeout", details: null, incidentId: null })
    .mockResolvedValueOnce({
      taskId: "accepted",
      status: "cancelled",
      progress: null,
      error: null,
    });
  const host = document.createElement("div");
  const root = createRoot(host);
  let controller!: ReturnType<typeof useBayesInferenceTask>;
  function Probe() {
    controller = useBayesInferenceTask();
    return null;
  }
  const draft = { formulaText: "original" } as BayesModelDraftDTO;
  try {
    await act(async () => root.render(createElement(Probe)));
    await act(async () => controller.run(draft, 90_000));
    expect(controller.phase).toBe("submission_unknown");
    draft.formulaText = "new edit";
    await act(async () => controller.retrySubmission());
    expect(controller.phase).toBe("cancelled");
    expect(mocks.submit.mock.calls[1]).toEqual(mocks.submit.mock.calls[0]);
    expect(mocks.submit.mock.calls[1][0].formulaText).toBe("original");
    expect(mocks.submit.mock.calls[1][1].timeoutMs).toBe(90_000);
  } finally {
    await act(async () => root.unmount());
  }
});
