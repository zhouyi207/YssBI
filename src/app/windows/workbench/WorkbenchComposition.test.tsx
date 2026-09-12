// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { WorkbenchComposition } from "./WorkbenchComposition";

const mocks = vi.hoisted(() => ({
  confirm: vi.fn(),
  destroy: vi.fn(async () => {}),
  callbacks: [] as Array<(event: { event: string; id: number; payload: null }) => Promise<void>>,
}));

vi.mock("@tauri-apps/api/window", async (importOriginal) => {
  const sdk = await importOriginal<typeof import("@tauri-apps/api/window")>();
  const native = {
    label: "main",
    listen: vi.fn(async (_event, callback) => {
      mocks.callbacks.push(callback);
      return () => {
        mocks.callbacks.splice(mocks.callbacks.indexOf(callback), 1);
      };
    }),
    destroy: mocks.destroy,
    onCloseRequested: sdk.Window.prototype.onCloseRequested,
  };
  return { ...sdk, getCurrentWindow: () => native };
});
vi.mock("@/features/application/initialization", () => ({
  useAppInitialization: () => ({ status: "loading" }),
}));
vi.mock("@/features/application/editor/editorPanelDirty", () => ({
  collectDirtyEditorPanels: () => [{ title: "Unsaved graph" }],
}));
vi.mock("@/features/core/ui/UIStore", () => ({ uiStore: { confirm3: mocks.confirm } }));
vi.mock("react-i18next", async (importOriginal) => ({
  ...(await importOriginal<typeof import("react-i18next")>()),
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock("./rootPanelRegistry", () => ({ rootPanelRegistry: {} }));
vi.mock("./rootPanelTabRenderer", () => ({ rootPanelTabRenderer: () => null }));
vi.mock("./menuContributionRegistry", () => ({ WorkbenchMenuContribution: () => null }));
vi.mock("./statusBarContributionRegistry", () => ({ WorkbenchStatusBarContribution: () => null }));

it("keeps the workbench open while confirmation is pending and after cancellation", async () => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  let cancel!: () => void;
  mocks.confirm.mockImplementation(
    () =>
      new Promise((resolve) => {
        cancel = () => resolve("cancel");
      }),
  );
  const host = document.createElement("div");
  const root = createRoot(host);
  try {
    await act(async () => root.render(<WorkbenchComposition />));
    // Use the installed SDK's close dispatch, where each allowed listener destroys the window.
    const pending = mocks.callbacks.map((callback) =>
      callback({ event: "tauri://close-requested", id: 1, payload: null }),
    );
    await vi.waitFor(() => expect(mocks.confirm).toHaveBeenCalledOnce());
    expect(mocks.destroy).not.toHaveBeenCalled();
    cancel();
    await Promise.all(pending);
    expect(mocks.destroy).not.toHaveBeenCalled();
  } finally {
    await act(async () => root.unmount());
  }
  expect(mocks.callbacks).toHaveLength(0);
});
