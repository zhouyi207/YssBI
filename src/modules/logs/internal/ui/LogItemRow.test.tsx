// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DiagnosticRecordDto } from "@/shared/types/domain/diagnostics";
import { LogItemRow } from "./LogItemRow";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const LOG: DiagnosticRecordDto = {
  streamId: "stream-1",
  sequence: 12,
  timestamp: "2026-08-11T12:34:56.000Z",
  level: "info",
  origin: "frontend",
  domain: "application",
  target: "chart",
  source: "chart",
  message: "Rendered chart successfully",
  fields: {},
};

describe("LogItemRow", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    window.getSelection()?.removeAllRanges();
    act(() => {
      root.unmount();
    });
    host.remove();
  });

  function renderRow(onClick = vi.fn()) {
    act(() => {
      root.render(<LogItemRow log={LOG} isSelected={false} onClick={onClick} />);
    });

    const button = host.querySelector("button");
    if (!button) throw new Error("Expected log row button");

    return { button, onClick };
  }

  it("renders all visible fields", () => {
    const { button } = renderRow();

    expect(button.textContent).toContain("12:34:56");
    expect(button.textContent).toContain("info");
    expect(button.textContent).toContain("APP");
    expect(button.textContent).toContain("[chart]");
    expect(button.textContent).toContain("Rendered chart successfully");
  });

  it("calls the callback once for an ordinary pointer click", () => {
    const { button, onClick } = renderRow();

    act(() => {
      button.dispatchEvent(new MouseEvent("click", { bubbles: true, detail: 1 }));
    });

    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("calls the callback once for a keyboard-style click", () => {
    const { button, onClick } = renderRow();

    act(() => {
      button.dispatchEvent(new MouseEvent("click", { bubbles: true, detail: 0 }));
    });

    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("suppresses the generated click after selecting message text", () => {
    const { button, onClick } = renderRow();
    const message = Array.from(button.querySelectorAll("span")).find(
      (span) => span.textContent === LOG.message,
    );
    const messageText = message?.firstChild;
    if (!messageText) throw new Error("Expected message text node");

    act(() => {
      const range = document.createRange();
      range.setStart(messageText, 0);
      range.setEnd(messageText, LOG.message.length);
      const selection = window.getSelection();
      selection?.removeAllRanges();
      selection?.addRange(range);

      button.dispatchEvent(new MouseEvent("click", { bubbles: true, detail: 1 }));
    });

    expect(onClick).not.toHaveBeenCalled();
  });

  it.each([
    ["an unmatched pointerup", new Event("pointerup", { bubbles: true })],
    ["a secondary pointerup", new PointerEvent("pointerup", { bubbles: true, button: 2 })],
  ])("allows a detail=0 click after %s with selected row text", (_, pointerUp) => {
    const { button, onClick } = renderRow();
    const message = Array.from(button.querySelectorAll("span")).find(
      (span) => span.textContent === LOG.message,
    );
    if (!message) throw new Error("Expected message element");

    act(() => {
      const range = document.createRange();
      range.selectNodeContents(message);
      const selection = window.getSelection();
      selection?.removeAllRanges();
      selection?.addRange(range);

      button.dispatchEvent(pointerUp);
      button.dispatchEvent(new MouseEvent("click", { bubbles: true, detail: 0 }));
    });

    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
