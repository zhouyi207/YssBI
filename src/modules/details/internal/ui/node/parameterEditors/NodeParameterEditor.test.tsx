// @vitest-environment happy-dom

import { act } from "react";
import { flushSync } from "react-dom";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { i18n } from "@/app/i18n";
import type { ApplyGraphDraftMutationOutcome } from "@/features/application/graphDraft/graphDraftCoordinator";
import { normalizeIpcError } from "@/services/ipc";
import type { ParameterEditorDto } from "@/shared/types/dto/editorProjection";
import { NodeParameterEditor } from "./NodeParameterEditor";

const { setNodeParameters } = vi.hoisted(() => ({ setNodeParameters: vi.fn() }));

vi.mock("@/features/application/editor/setNodeParameters", () => ({ setNodeParameters }));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const graphPath = "events/Main.yssbi-event";
const nodeId = "constant-node";
const appliedOutcome: ApplyGraphDraftMutationOutcome = {
  status: "applied",
  result: {} as never,
  insertedNodeIds: [],
};
let container: HTMLDivElement;
let root: Root;

function parameter(
  editor: ParameterEditorDto["editor"],
  value: unknown,
  valueType: ParameterEditorDto["valueType"],
  multiline = false,
): ParameterEditorDto {
  return {
    key: "value",
    display: { title: "Value", description: null },
    editor,
    presentation: "detailPanel",
    valueType,
    multiline,
    value,
    configuration: null,
  };
}

function renderEditor(projected: ParameterEditorDto): void {
  act(() =>
    root.render(
      <NodeParameterEditor
        graphPath={graphPath}
        nodeId={nodeId}
        locale="en-US"
        parameter={projected}
        diagnostics={[]}
        formatFallback={String}
      />,
    ),
  );
}

function input(): HTMLInputElement {
  const element = container.querySelector("input");
  if (!(element instanceof HTMLInputElement)) throw new Error("missing input");
  return element;
}

function setControlValue(element: HTMLInputElement | HTMLTextAreaElement, value: string): void {
  act(() => {
    const prototype =
      element instanceof HTMLTextAreaElement
        ? HTMLTextAreaElement.prototype
        : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(prototype, "value")?.set?.call(element, value);
    element.dispatchEvent(new Event("input", { bubbles: true }));
    element.dispatchEvent(new Event("change", { bubbles: true }));
  });
}

async function flushPromises(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

beforeEach(() => {
  container = document.createElement("div");
  document.body.appendChild(container);
  root = createRoot(container);
  setNodeParameters.mockReset();
  setNodeParameters.mockResolvedValue(appliedOutcome);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

describe("NodeParameterEditor ordinary controls", () => {
  it("can choose the first schema option when a required parameter is unset", async () => {
    renderEditor({
      ...parameter("select", null, { kind: "String" }),
      configuration: { kind: "selectOptions", options: ["amount", "quantity"] },
    });
    const select = container.querySelector("select");
    if (!(select instanceof HTMLSelectElement)) throw new Error("missing select");
    expect(select.value).toBe("");
    act(() => {
      select.value = "amount";
      select.dispatchEvent(new Event("change", { bubbles: true }));
    });
    await flushPromises();
    expect(setNodeParameters).toHaveBeenCalledWith(
      expect.objectContaining({ parameters: { value: "amount" } }),
    );
  });

  it("commits a toggle immediately through setNodeParameters", async () => {
    renderEditor(parameter("toggle", false, { kind: "Boolean" }));

    const toggle = container.querySelector('[role="switch"]');
    if (!toggle) throw new Error("missing switch");
    act(() => toggle.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    await flushPromises();

    expect(setNodeParameters).toHaveBeenCalledWith({
      graphPath,
      nodeId,
      locale: "en-US",
      parameters: { value: true },
    });
  });

  it("describes an invalid numeric draft with a field-level error", () => {
    renderEditor(parameter("number", 1, { kind: "Int64" }));

    setControlValue(input(), "1.5");
    act(() => input().dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })));

    const error = container.querySelector<HTMLElement>('[role="alert"]');
    expect(error?.textContent).toContain(i18n.t("notifications.parameter.enterInteger"));
    expect(input().getAttribute("aria-describedby")).toBe(error?.id);
    expect(setNodeParameters).not.toHaveBeenCalled();
  });

  it.each([
    [{ kind: "Int64" }, "1.5", false],
    [{ kind: "Float64" }, "1.5", true],
  ] as const)(
    "uses projected %s semantics for numeric commits",
    async (valueType, draft, shouldCommit) => {
      renderEditor(parameter("number", 1, valueType));
      expect(container.querySelector("select")).toBeNull();
      const input = container.querySelector("input");
      if (!(input instanceof HTMLInputElement)) throw new Error("missing input");

      setControlValue(input, draft);
      act(() => input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })));
      await flushPromises();

      if (shouldCommit) {
        expect(setNodeParameters).toHaveBeenCalledWith({
          graphPath,
          nodeId,
          locale: "en-US",
          parameters: { value: 1.5 },
        });
      } else {
        expect(setNodeParameters).not.toHaveBeenCalled();
      }
    },
  );

  it.each(["text", "select"] as const)(
    "edits %s without an option list as text",
    async (editor) => {
      renderEditor(parameter(editor, "old", { kind: "String" }));
      const input = container.querySelector("input");
      if (!(input instanceof HTMLInputElement)) throw new Error("missing input");

      setControlValue(input, "new");
      act(() => input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })));
      await flushPromises();
      expect(setNodeParameters).toHaveBeenLastCalledWith(
        expect.objectContaining({ parameters: { value: "new" } }),
      );

      setControlValue(input, "draft");
      act(() =>
        input.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true })),
      );
      expect(input.value).toBe("old");
    },
  );

  it("does not write a text draft per keystroke", () => {
    renderEditor(parameter("text", "old", { kind: "String" }));

    setControlValue(input(), "new");

    expect(input().value).toBe("new");
    expect(setNodeParameters).not.toHaveBeenCalled();
  });

  it("commits a numeric draft on blur", async () => {
    renderEditor(parameter("number", 1, { kind: "Float64" }));

    setControlValue(input(), "2.5");
    act(() => input().dispatchEvent(new FocusEvent("focusout", { bubbles: true })));
    await flushPromises();

    expect(setNodeParameters).toHaveBeenCalledWith({
      graphPath,
      nodeId,
      locale: "en-US",
      parameters: { value: 2.5 },
    });
  });

  it("resets an invalid numeric blur and syncs a later projected value", () => {
    const initial = parameter("number", 1, { kind: "Int64" });
    renderEditor(initial);

    setControlValue(input(), "1.5");
    act(() => input().dispatchEvent(new FocusEvent("focusout", { bubbles: true })));
    expect(input().value).toBe("1");
    expect(setNodeParameters).not.toHaveBeenCalled();

    renderEditor({ ...initial, value: 2 });
    expect(input().value).toBe("2");
  });

  it("synchronously blocks blur from committing an active draft when a newer projection renders", async () => {
    renderEditor(parameter("text", "old", { kind: "String" }));
    setControlValue(input(), "draft");

    act(() => {
      flushSync(() =>
        root.render(
          <NodeParameterEditor
            graphPath={graphPath}
            nodeId={nodeId}
            locale="en-US"
            parameter={parameter("text", "projected", { kind: "String" })}
            diagnostics={[]}
            formatFallback={String}
          />,
        ),
      );
      input().dispatchEvent(new FocusEvent("focusout", { bubbles: true }));
    });
    expect(input().value).toBe("projected");
    await flushPromises();

    expect(setNodeParameters).not.toHaveBeenCalled();
  });

  it("guards synchronously against Enter plus blur duplicate commits", async () => {
    let resolveMutation: (outcome: ApplyGraphDraftMutationOutcome) => void = () => undefined;
    setNodeParameters.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveMutation = resolve;
        }),
    );
    renderEditor(parameter("text", "old", { kind: "String" }));
    setControlValue(input(), "draft");

    act(() => {
      input().dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
      input().dispatchEvent(new FocusEvent("focusout", { bubbles: true }));
    });

    expect(setNodeParameters).toHaveBeenCalledOnce();
    resolveMutation(appliedOutcome);
    await flushPromises();
  });

  it("does not submit an invalid numeric draft on blur", () => {
    renderEditor(parameter("number", 1, { kind: "Int64" }));
    setControlValue(input(), "1.5");

    act(() => input().dispatchEvent(new FocusEvent("focusout", { bubbles: true })));

    expect(setNodeParameters).not.toHaveBeenCalled();
  });

  it.each(["stale", "conflict"] as const)(
    "restores the latest projection when mutation resolves %s",
    async (status) => {
      setNodeParameters.mockResolvedValueOnce({ status });
      const initial = parameter("text", "old", { kind: "String" });
      renderEditor(initial);
      setControlValue(input(), "draft");
      act(() =>
        input().dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })),
      );

      renderEditor({ ...initial, value: "latest projection" });
      await flushPromises();

      expect(input().value).toBe("latest projection");
    },
  );

  it("shows an IPC update failure beside the field without exposing backend details", async () => {
    setNodeParameters.mockRejectedValueOnce(
      normalizeIpcError("transform_graph_draft", {
        code: "parameter_update_failed",
        details: { debug: "raw backend parameter failure" },
        incidentId: "incident-parameter-42",
      }),
    );
    renderEditor(parameter("number", 1, { kind: "Int64" }));

    setControlValue(input(), "2");
    act(() => input().dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })));
    await flushPromises();

    const error = container.querySelector<HTMLElement>('[role="alert"]');
    expect(error?.textContent).toContain("parameter_update_failed");
    expect(error?.textContent).toContain("incident-parameter-42");
    expect(error?.textContent).not.toContain("raw backend parameter failure");
    expect(input().getAttribute("aria-describedby")).toBe(error?.id);
  });

  it("restores the latest projected value when mutation rejects", async () => {
    let rejectMutation: (reason: Error) => void = () => undefined;
    setNodeParameters.mockImplementationOnce(
      () =>
        new Promise((_, reject) => {
          rejectMutation = reject;
        }),
    );
    renderEditor(parameter("text", "old", { kind: "String" }));
    setControlValue(input(), "draft");
    act(() => input().dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true })));

    renderEditor(parameter("text", "latest projection", { kind: "String" }));
    expect(input().value).toBe("latest projection");
    rejectMutation(new Error("backend rejected value"));
    await flushPromises();

    expect(input().value).toBe("latest projection");
  });

  it("restores the toggle state when its mutation rejects", async () => {
    setNodeParameters.mockRejectedValueOnce(new Error("toggle rejected"));
    renderEditor(parameter("toggle", false, { kind: "Boolean" }));

    const toggle = container.querySelector('[role="switch"]');
    if (!toggle) throw new Error("missing switch");
    act(() => toggle.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    await flushPromises();

    expect(toggle.getAttribute("data-state")).toBe("unchecked");
  });

  it("renders multiline text in a textarea and commits it on blur", async () => {
    renderEditor(parameter("text", "old", { kind: "String" }, true));
    const textarea = container.querySelector("textarea");
    if (!(textarea instanceof HTMLTextAreaElement)) throw new Error("missing textarea");

    setControlValue(textarea, "line one\nline two");
    act(() => textarea.dispatchEvent(new FocusEvent("focusout", { bubbles: true })));
    await flushPromises();

    expect(setNodeParameters).toHaveBeenCalledWith(
      expect.objectContaining({
        parameters: { value: "line one\nline two" },
      }),
    );
  });

  it.each(["auto", "select", "resource"] as const)("keeps %s parameters read-only", (editor) => {
    renderEditor(parameter(editor, "projected", null));

    expect(container.textContent).toContain("projected");
    expect(container.querySelector('input, textarea, [role="switch"]')).toBeNull();
  });
});
