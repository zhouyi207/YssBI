import { Children, isValidElement, type ReactElement } from "react";
import { createDataSignaturePin } from "@/shared/types/domain/functionSignaturePin";
import { describe, expect, it, vi } from "vitest";
import { PinEditor } from "../shared/PinEditor";
import { DetailReadonlyField } from "../shared/DetailForm";
import { FunctionDetailPanel } from "./FunctionDetailPanel";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

function findAllByType(root: ReactElement, type: unknown): ReactElement[] {
  const matches: ReactElement[] = [];

  function visit(node: unknown) {
    if (!isValidElement(node)) return;
    if (node.type === type) {
      matches.push(node);
    }
    Children.forEach((node.props as { children?: unknown }).children, visit);
  }

  visit(root);
  return matches;
}

describe("FunctionDetailPanel", () => {
  it("renders the resource name as read-only while keeping signature edits available", () => {
    const onSignatureChange = vi.fn();
    const element = FunctionDetailPanel({
      fn: {
        name: "Compute",
        inputs: [createDataSignaturePin("input-1", "Value", { kind: "Int64" })],
        outputs: [createDataSignaturePin("output-1", "Result", { kind: "Float64" })],
      },
      onSignatureChange,
    }) as ReactElement;

    const nameField = findAllByType(element, DetailReadonlyField)[0];
    expect((nameField.props as { children?: unknown }).children).toBe("Compute");

    const pinEditors = findAllByType(element, PinEditor);
    (pinEditors[0].props as { onChange: (pins: unknown[]) => void }).onChange([
      createDataSignaturePin("input-2", "Next", { kind: "String" }),
    ]);
    (pinEditors[1].props as { onChange: (pins: unknown[]) => void }).onChange([
      createDataSignaturePin("output-2", "Done", { kind: "Boolean" }),
    ]);

    expect(onSignatureChange).toHaveBeenNthCalledWith(1, {
      inputs: [createDataSignaturePin("input-2", "Next", { kind: "String" })],
    });
    expect(onSignatureChange).toHaveBeenNthCalledWith(2, {
      outputs: [createDataSignaturePin("output-2", "Done", { kind: "Boolean" })],
    });
  });
});
