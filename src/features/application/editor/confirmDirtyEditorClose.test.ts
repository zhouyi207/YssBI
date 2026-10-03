import { beforeEach, expect, it, vi } from "vitest";
import { startProjectLifecycle } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { confirmDirtyEditorClose } from "./confirmDirtyEditorClose";

const mocks = vi.hoisted(() => ({
  confirm: vi.fn(),
  settle: vi.fn(),
  save: vi.fn(),
  showError: vi.fn(),
  logError: vi.fn(),
}));

vi.mock("@/features/core/ui/UIStore", () => ({ uiStore: { confirm3: mocks.confirm } }));
vi.mock("./settleEditorFileEdits", () => ({ settleEditorFileEdits: mocks.settle }));
vi.mock("./editorPanelDirty", () => ({
  collectDirtyEditorPanels: () => [
    { title: "First", resourceRef: "docs/First.md", resourceKind: "doc" },
    { title: "Second", resourceRef: "docs/Second.md", resourceKind: "doc" },
  ],
}));
vi.mock("@/features/application/resource/resourceActions", () => ({
  saveFileResource: mocks.save,
}));
vi.mock("./blockingErrorDialog", () => ({ showBlockingIpcError: mocks.showError }));
vi.mock("@/utils/frontendLogger", () => ({ logger: { app: { error: mocks.logError } } }));
vi.mock("@/app/i18n", () => ({ i18n: { t: (key: string) => key } }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((finish) => {
    resolve = finish;
  });
  return { promise, resolve };
}

beforeEach(() => {
  vi.clearAllMocks();
  startProjectLifecycle("project-a");
  mocks.confirm.mockResolvedValue("confirm");
  mocks.settle.mockResolvedValue(undefined);
  mocks.save.mockResolvedValue(true);
});

it("does not start saving after the close owner expires during confirmation", async () => {
  const decision = deferred<"confirm">();
  mocks.confirm.mockReturnValueOnce(decision.promise);
  let active = true;
  const closing = confirmDirtyEditorClose(() => active);
  await vi.waitFor(() => expect(mocks.confirm).toHaveBeenCalledOnce());

  active = false;
  decision.resolve("confirm");

  await expect(closing).resolves.toBe(false);
  expect(mocks.save).not.toHaveBeenCalled();
  expect(mocks.showError).not.toHaveBeenCalled();
});

it("lets an issued save settle without starting the next file after the close owner expires", async () => {
  const firstSave = deferred<boolean>();
  mocks.save.mockReturnValueOnce(firstSave.promise);
  let active = true;
  const closing = confirmDirtyEditorClose(() => active);
  await vi.waitFor(() => expect(mocks.save).toHaveBeenCalledOnce());

  active = false;
  firstSave.resolve(true);

  await expect(closing).resolves.toBe(false);
  expect(mocks.save).toHaveBeenCalledExactlyOnceWith("docs/First.md", "doc");
  expect(mocks.showError).not.toHaveBeenCalled();
});
