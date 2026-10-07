import { beforeEach, expect, it, vi } from "vitest";
import { pendingUiIntents, settleUiIntent } from "@/services/workbench/presentationService";
import type { UiIntentReceipt } from "@/shared/types/domain/uiPresentation";
import { createUiIntentDelivery } from "./uiIntentDelivery";

vi.mock("@/services/workbench/presentationService", () => ({
  pendingUiIntents: vi.fn(),
  settleUiIntent: vi.fn(),
}));

const receipt: UiIntentReceipt = {
  id: "00000000-0000-0000-0000-000000000001",
  intent: { kind: "showPanel", panel: "assistant" },
  status: "pending",
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(settleUiIntent).mockResolvedValue(true);
});

it("recovers a second channel gap arriving while an older pending snapshot is in flight", async () => {
  const first = deferred<UiIntentReceipt[]>();
  vi.mocked(pendingUiIntents).mockReturnValueOnce(first.promise).mockResolvedValueOnce([receipt]);
  const execute = vi.fn(async () => true);
  const failure = vi.fn();
  const delivery = createUiIntentDelivery("project", () => true, execute, failure);
  const recovery = delivery.recover();
  void delivery.recover(true);
  void delivery.recover(true);
  first.resolve([]);
  await recovery;
  expect(pendingUiIntents).toHaveBeenCalledTimes(2);
  await vi.waitFor(() =>
    expect(settleUiIntent).toHaveBeenLastCalledWith("project", receipt.id, "applied"),
  );
  expect(execute).toHaveBeenCalledExactlyOnceWith(receipt.intent);
  expect(failure).not.toHaveBeenCalled();
});

it("retries a new gap after a failed read but drops recovery work after the binding expires", async () => {
  const first = deferred<UiIntentReceipt[]>();
  const second = deferred<UiIntentReceipt[]>();
  vi.mocked(pendingUiIntents)
    .mockReturnValueOnce(first.promise)
    .mockReturnValueOnce(second.promise);
  let current = true;
  const execute = vi.fn(async () => true);
  const failure = vi.fn();
  const delivery = createUiIntentDelivery("project", () => current, execute, failure);
  const recovery = delivery.recover();
  void delivery.recover(true);
  first.reject(new Error("read failed"));
  await vi.waitFor(() => expect(pendingUiIntents).toHaveBeenCalledTimes(2));
  const finalRead = delivery.recover(true);
  current = false;
  second.resolve([receipt]);
  await Promise.all([recovery, finalRead]);
  expect(pendingUiIntents).toHaveBeenCalledTimes(2);
  expect(settleUiIntent).not.toHaveBeenCalled();
  expect(execute).not.toHaveBeenCalled();
  expect(failure).toHaveBeenCalledOnce();
});

it("recovers pending receipts dropped while an expired delivery batch still fills the queue", async () => {
  const held = deferred<boolean>();
  const id = (index: number) => `00000000-0000-0000-0000-${index.toString(16).padStart(12, "0")}`;
  const late: UiIntentReceipt = {
    ...receipt,
    id: id(129),
    intent: { kind: "showPanel", panel: "problems" },
  };
  vi.mocked(pendingUiIntents).mockResolvedValue([late]);
  vi.mocked(settleUiIntent).mockImplementation(
    async (_project, id, status) => status !== "claimed" || id === receipt.id || id === late.id,
  );
  const execute = vi.fn(async () => true).mockReturnValueOnce(held.promise);
  const failure = vi.fn();
  const delivery = createUiIntentDelivery("project", () => true, execute, failure);
  const batch = [delivery.accept(receipt)];
  for (let index = 1; index < 128; index++)
    batch.push(delivery.accept({ ...receipt, id: id(index + 1) }));
  await delivery.accept(late);
  await delivery.accept(late);
  expect(pendingUiIntents).not.toHaveBeenCalled();
  held.resolve(true);
  await Promise.all(batch);
  await vi.waitFor(() =>
    expect(settleUiIntent).toHaveBeenLastCalledWith("project", late.id, "applied"),
  );
  expect(pendingUiIntents).toHaveBeenCalledExactlyOnceWith("project");
  expect(execute.mock.calls).toEqual([[receipt.intent], [late.intent]]);
  expect(failure).not.toHaveBeenCalled();
});
