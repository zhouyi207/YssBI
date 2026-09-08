// @vitest-environment happy-dom

import { act, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  type ManagedProject,
  type ProjectPickerLifecycleActionOutcome,
} from "@/features/application/project";
import { ProjectService } from "@/services/project/projectService";
import { DeleteProjectConfirmDialog } from "./DeleteProjectConfirmDialog";
import { NewProjectModal } from "./NewProjectModal";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock("react-i18next", () => ({
  initReactI18next: { type: "3rdParty", init: vi.fn() },
  useTranslation: () => ({ t: (key: string) => key }),
}));

const project: ManagedProject = {
  id: "project-a",
  name: "Project A",
  path: "C:/Project A/metadata.yssbi",
  lastOpenedAt: "2026-08-16T00:00:00Z",
};

function PendingProjectDialog({
  action,
  onSubmit,
}: {
  action: "create" | "delete";
  onSubmit: () => Promise<ProjectPickerLifecycleActionOutcome>;
}) {
  const [open, setOpen] = useState(true);

  return action === "create" ? (
    <NewProjectModal open={open} onOpenChange={setOpen} onCreate={onSubmit} />
  ) : (
    <DeleteProjectConfirmDialog
      project={open ? project : null}
      onOpenChange={setOpen}
      onConfirm={onSubmit}
    />
  );
}

describe("project picker modal dismissal", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.spyOn(ProjectService, "defaultProjectParentDirectory").mockResolvedValue("C:/Projects");
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    vi.restoreAllMocks();
  });

  it.each(["create", "delete"] as const)(
    "dismisses the %s dialog on backdrop interaction while the submitted operation is pending",
    async (action) => {
      let complete!: (outcome: ProjectPickerLifecycleActionOutcome) => void;
      const onSubmit = vi.fn(
        () =>
          new Promise<ProjectPickerLifecycleActionOutcome>((resolve) => {
            complete = resolve;
          }),
      );

      await act(async () => {
        root.render(<PendingProjectDialog action={action} onSubmit={onSubmit} />);
      });
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 0));
      });

      const submitLabel =
        action === "create"
          ? "projectPicker.newProjectModal.create"
          : "projectPicker.deleteProjectConfirm.confirm";
      const submit = [...document.querySelectorAll("button")].find(
        (button) => button.textContent === submitLabel,
      );
      if (!submit) throw new Error("missing submit button");
      act(() => submit.click());
      expect(onSubmit).toHaveBeenCalledOnce();
      expect(submit.disabled).toBe(true);

      const overlay = document.querySelector('[data-slot="dialog-overlay"]');
      if (!overlay) throw new Error("missing dialog backdrop");
      act(() => {
        overlay.dispatchEvent(
          new PointerEvent("pointerdown", {
            bubbles: true,
            cancelable: true,
            pointerType: "mouse",
          }),
        );
        overlay.dispatchEvent(
          new PointerEvent("pointerup", { bubbles: true, pointerType: "mouse" }),
        );
        overlay.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
      });
      expect(document.querySelector('[role="dialog"]')).toBeNull();

      await act(async () => complete({ status: "stale" }));
      expect(document.querySelector('[role="dialog"]')).toBeNull();
      expect(onSubmit).toHaveBeenCalledOnce();
    },
  );
});
