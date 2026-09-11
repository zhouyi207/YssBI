// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { DatabaseService } from "@/services/database/databaseService";
import { ImportModal } from "./ImportModal";

vi.mock("@/services/database/databaseService", () => ({
  DatabaseService: { listSampleDatasets: vi.fn() },
}));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
    i18n: { language: "en-US" },
  }),
}));

let host: HTMLDivElement;
let root: Root;
const onClose = vi.fn();
const onSelect = vi.fn();
const onImportSample = vi.fn<(id: string, version: number) => Promise<void>>();
const sample = {
  id: "iris",
  name: "Iris",
  version: 1,
  rowCount: 150,
  columnCount: 5,
  byteSize: 2861,
};
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function button(text: string) {
  const found = [...document.querySelectorAll("button")].find(
    (element) => element.textContent === text,
  );
  if (!found) throw new Error(`missing button: ${text}`);
  return found;
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(DatabaseService.listSampleDatasets).mockResolvedValue([sample]);
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
  act(() => root.render(<ImportModal options={{ onSelect, onImportSample }} onClose={onClose} />));
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
  document.body.innerHTML = "";
});

it("loads the catalog only on selection and allows retry after a discovery failure", async () => {
  expect(DatabaseService.listSampleDatasets).not.toHaveBeenCalled();
  vi.mocked(DatabaseService.listSampleDatasets).mockRejectedValueOnce(
    new Error("private filesystem detail"),
  );
  await act(async () => button("importModal.categories.samples").click());
  expect(document.querySelector("[role=alert]")?.textContent).toContain(
    "importModal.samples.failed",
  );
  expect(document.body.textContent).not.toContain("private filesystem detail");
  await act(async () => button("importModal.samples.retry").click());
  expect(document.body.textContent).toContain("Iris");
  expect(onImportSample).not.toHaveBeenCalled();
});

it("keeps a failed import open, blocks duplicate clicks and closes only after success", async () => {
  await act(async () => button("importModal.categories.samples").click());
  let rejectImport!: (error: Error) => void;
  onImportSample.mockImplementationOnce(
    () =>
      new Promise<void>((_, reject) => {
        rejectImport = reject;
      }),
  );
  act(() => {
    const importButton = button("importModal.samples.import");
    importButton.click();
    importButton.click();
  });
  expect(onImportSample).toHaveBeenCalledExactlyOnceWith("iris", 1);
  expect(onClose).not.toHaveBeenCalled();
  expect(button("importModal.categories.file").disabled).toBe(true);
  await act(async () => rejectImport(new Error("private database detail")));
  expect(onClose).not.toHaveBeenCalled();
  expect(document.body.textContent).not.toContain("private database detail");
  expect(button("importModal.samples.import").disabled).toBe(false);
  let finishImport!: () => void;
  onImportSample.mockImplementationOnce(
    () =>
      new Promise<void>((resolve) => {
        finishImport = resolve;
      }),
  );
  act(() => button("importModal.samples.import").click());
  expect(onClose).not.toHaveBeenCalled();
  await act(async () => finishImport());
  expect(onClose).toHaveBeenCalledOnce();
});
