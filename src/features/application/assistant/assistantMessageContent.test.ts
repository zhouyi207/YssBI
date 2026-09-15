import { expect, it } from "vitest";
import {
  appendAssistantText,
  finishAssistantText,
  updateAssistantContent,
  type AssistantMessageContent,
} from "./assistantMessageContent";

it("replays interleaved text and tool events without moving content or overwriting the transcript", () => {
  const replay = () => {
    let content: AssistantMessageContent = appendAssistantText([], "先检查");
    content = appendAssistantText(content, "数据。");
    content = updateAssistantContent(content, [], null, [
      { invocationId: "pending-3", capabilityId: "inspect_graph", state: "running" },
    ]);
    content = updateAssistantContent(content, [], null, [
      { invocationId: "tool-1", capabilityId: "inspect_graph", state: "running" },
    ]);
    content = appendAssistantText(content, "已发现两列。");
    content = updateAssistantContent(content, [], null, [
      { invocationId: "tool-1", capabilityId: "inspect_graph", state: "completed" },
    ]);
    expect(content.map((part) => part.type)).toEqual(["text", "tool-call", "text"]);
    expect(content[0]).toEqual({ type: "text", text: "先检查数据。" });
    expect(content[1]).toMatchObject({ toolCallId: "tool-1", result: { status: "completed" } });
    expect(finishAssistantText(content, "已发现两列。")).toBe(content);
    return content;
  };
  expect(replay()).toEqual(replay());
  expect(finishAssistantText([], "Restored")).toEqual([{ type: "text", text: "Restored" }]);
});

it("retains partial text and tool position on cancellation and appends structured evidence in event order", () => {
  let content = appendAssistantText([], "开始检查。");
  content = updateAssistantContent(content, [], { researchQuestion: "检查数据" }, []);
  content = updateAssistantContent(content, [], null, [
    { invocationId: "tool-1", capabilityId: "inspect_graph", state: "running" },
  ]);
  content = appendAssistantText(content, "部分结果");
  content = updateAssistantContent(content, [], null, [
    { invocationId: "tool-1", capabilityId: "inspect_graph", state: "cancelled" },
  ]);
  expect(content.map((part) => part.type)).toEqual(["text", "data", "tool-call", "text"]);
  expect(content[2]).toMatchObject({ result: { status: "cancelled" }, isError: true });
  expect(content[3]).toEqual({ type: "text", text: "部分结果" });
});
