// @vitest-environment happy-dom

import { act, type ReactNode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { normalizeIpcError } from "@/services/ipc";
import { ProjectService } from "@/services/project/projectService";
import {
  projectPickerErrorPresentation,
  type ManagedProject,
  type ProjectPickerPageIssue,
} from "@/features/application/project";
import { NewProjectModal } from "./NewProjectModal";
import { DeleteProjectConfirmDialog } from "./DeleteProjectConfirmDialog";
import { ProjectPickerPageIssueAlert } from "./ProjectPickerPageIssueAlert";

const openPathDialog = vi.hoisted(() => vi.fn());

vi.mock("react-i18next", () => ({
  initReactI18next: { type: "3rdParty", init: vi.fn() },
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key,
  }),
}));

vi.mock("@/components/ui/dialog", () => ({
  Dialog: ({ open, children }: { open: boolean; children: ReactNode }) =>
    open ? <div data-testid="dialog">{children}</div> : null,
  DialogContent: ({ children }: { children: ReactNode }) => <section>{children}</section>,
  DialogDescription: ({ children }: { children: ReactNode }) => <p>{children}</p>,
  DialogFooter: ({ children }: { children: ReactNode }) => <footer>{children}</footer>,
  DialogHeader: ({ children }: { children: ReactNode }) => <header>{children}</header>,
  DialogTitle: ({ children }: { children: ReactNode }) => <h2>{children}</h2>,
}));

vi.mock("@/services/platform/pathDialog", () => ({
  openPathDialog,
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function ipcFailure(command: string, code: string, incidentId: string | null = null) {
  return projectPickerErrorPresentation(
    normalizeIpcError(command, {
      code,
      details: null,
      incidentId,
    }),
  );
}

function click(element: Element | null): void {
  if (!element) throw new Error("missing test element");
  act(() => element.dispatchEvent(new MouseEvent("click", { bubbles: true })));
}

async function flush(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe("project picker visible feedback", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.restoreAllMocks();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    vi.spyOn(ProjectService, "defaultProjectParentDirectory").mockResolvedValue("C:/Projects");
    openPathDialog.mockResolvedValue({ ok: true, value: null });
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("keeps the new-project dialog open when creation fails", async () => {
    const onOpenChange = vi.fn();
    const onCreate = vi.fn(async () => ({
      status: "failed" as const,
      error: ipcFailure("create_project", "invalid_project_root", "incident-create"),
    }));

    await act(async () => {
      root.render(<NewProjectModal open onOpenChange={onOpenChange} onCreate={onCreate} />);
      await Promise.resolve();
    });

    const create = [...host.querySelectorAll("button")].find(
      (button) => button.textContent === "projectPicker.newProjectModal.create",
    );
    click(create ?? null);
    await flush();

    expect(onCreate).toHaveBeenCalledOnce();
    expect(onOpenChange).not.toHaveBeenCalled();
    expect(host.querySelector('[data-testid="dialog"]')).not.toBeNull();
    expect(host.querySelector('[role="alert"]')).not.toBeNull();
    expect(host.textContent).toContain("invalid_project_root");
    expect(host.textContent).toContain("incident-create");
  });

  it("maps path dialog failures without exposing native error text", async () => {
    openPathDialog.mockResolvedValueOnce({
      ok: false,
      failure: { operation: "openPathDialog", code: "operationFailed" },
    });

    await act(async () => {
      root.render(<NewProjectModal open onOpenChange={vi.fn()} onCreate={vi.fn()} />);
      await Promise.resolve();
    });

    const browse = [...host.querySelectorAll("button")].find(
      (button) => button.textContent === "projectPicker.newProjectModal.browse",
    );
    click(browse ?? null);
    await flush();

    expect(host.textContent).toContain("unknown_error");
  });

  it("keeps the delete dialog open when deletion fails", async () => {
    const project: ManagedProject = {
      id: "project-a",
      name: "Project A",
      path: "C:/Project A/metadata.yssbi",
      lastOpenedAt: "2026-08-16T00:00:00Z",
    };
    const onOpenChange = vi.fn();
    const onConfirm = vi.fn(async () => ({
      status: "failed" as const,
      error: ipcFailure("delete_registered_project_files", "project_not_found"),
    }));

    act(() =>
      root.render(
        <DeleteProjectConfirmDialog
          project={project}
          onOpenChange={onOpenChange}
          onConfirm={onConfirm}
        />,
      ),
    );

    const confirm = [...host.querySelectorAll("button")].find(
      (button) => button.textContent === "projectPicker.deleteProjectConfirm.confirm",
    );
    click(confirm ?? null);
    await flush();

    expect(onConfirm).toHaveBeenCalledWith(project);
    expect(onOpenChange).not.toHaveBeenCalled();
    expect(host.querySelector('[data-testid="dialog"]')).not.toBeNull();
    expect(host.querySelector('[role="alert"]')).not.toBeNull();
    expect(host.textContent).toContain("project_not_found");
  });

  it("renders a dismissible, retryable page alert for a typed issue", () => {
    const onDismiss = vi.fn();
    const onRetry = vi.fn();
    const issue: ProjectPickerPageIssue = {
      kind: "failure",
      operation: "refresh",
      error: ipcFailure("list_registered_projects", "internal_error", "incident-refresh"),
    };

    act(() =>
      root.render(
        <ProjectPickerPageIssueAlert issue={issue} onDismiss={onDismiss} onRetry={onRetry} />,
      ),
    );

    expect(host.querySelector('[role="alert"]')).not.toBeNull();
    expect(host.textContent).toContain("internal_error");
    expect(host.textContent).toContain("incident-refresh");

    const retry = [...host.querySelectorAll("button")].find(
      (button) => button.textContent === "common.refresh",
    );
    click(retry ?? null);
    click(host.querySelector('button[aria-label="common.close"]'));

    expect(onRetry).toHaveBeenCalledOnce();
    expect(onDismiss).toHaveBeenCalledOnce();
  });
});
