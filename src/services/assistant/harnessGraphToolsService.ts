import { Channel } from "@tauri-apps/api/core";
import { invokeCommand } from "@/services/ipc";
import { trackChannel, untrackChannel } from "@/services/devHmrIpc";
import { clearChannelMessageHandler } from "@/shared/platform/tauriWebview";
import { createExecutionStreamDrain } from "@/services/project/executionChannelDrain";
import {
  parseCompileGraphDraftDto,
  parseGraphDraftSaveDto,
  parseGraphDraftTransformDto,
} from "@/shared/types/dto/editorMutationWireParser";
import type {
  CompileGraphDraftDto,
  GraphDraftSaveDto,
  GraphDraftTransformDto,
  GraphDocumentDto,
} from "@/shared/types/domain/editorMutation";
import type { RunEvent, RunOutputChannelEvent } from "@/shared/types/domain/runEvent";

const GRAPH_TOOLS = new Set([
  "inspect_graph",
  "apply_graph_edit",
  "compile_graph",
  "execute_graph",
  "save_graph",
]);
export interface HarnessGraphToolRequest {
  readonly requestId: string;
  readonly sessionId: string;
  readonly projectInstanceId: string;
  readonly graphPath: string;
  readonly capabilityId: string;
}
export type HarnessGraphUpdate =
  | { type: "none" }
  | {
      type: "execution";
      update: { terminalEventSent: boolean; status: "succeeded" | "failed" | "cancelled" };
    }
  | { type: "draft"; update: GraphDraftTransformDto }
  | { type: "compilation"; update: CompileGraphDraftDto }
  | { type: "saved"; update: GraphDraftSaveDto };

function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error("Invalid graph tool payload");
  return value as Record<string, unknown>;
}
export function parseHarnessGraphToolRequest(value: unknown): HarnessGraphToolRequest {
  const source = record(value);
  for (const key of ["requestId", "sessionId", "projectInstanceId", "graphPath", "capabilityId"]) {
    if (typeof source[key] !== "string" || source[key].length === 0)
      throw new Error("Invalid graph tool request");
  }
  if (!GRAPH_TOOLS.has(source.capabilityId as string)) throw new Error("Unknown graph tool");
  return source as unknown as HarnessGraphToolRequest;
}
function parseUpdate(value: unknown, projectInstanceId: string): HarnessGraphUpdate {
  const source = record(value);
  switch (source.type) {
    case "none":
      return { type: "none" };
    case "draft":
      return { type: "draft", update: parseGraphDraftTransformDto(source.update) };
    case "compilation":
      return { type: "compilation", update: parseCompileGraphDraftDto(source.update) };
    case "saved":
      return { type: "saved", update: parseGraphDraftSaveDto(source.update, projectInstanceId) };
    case "execution": {
      const update = record(source.update);
      const status = update.status;
      if (
        typeof update.terminalEventSent !== "boolean" ||
        (status !== "succeeded" && status !== "failed" && status !== "cancelled")
      )
        throw new Error("Invalid execution tool update");
      return { type: "execution", update: { terminalEventSent: update.terminalEventSent, status } };
    }
    default:
      throw new Error("Invalid graph tool update");
  }
}

export class HarnessGraphToolsService {
  static async subscribe(
    sessionId: string,
    onRequest: (request: HarnessGraphToolRequest) => Promise<void>,
  ): Promise<{ close(): Promise<void> }> {
    let subscriptionId: string | null = null;
    let closed = false;
    const cleanup = () => {
      closed = true;
      untrackChannel(channel);
      clearChannelMessageHandler(channel);
    };
    const channel = trackChannel(new Channel<unknown>(), () => {
      cleanup();
      if (subscriptionId)
        void invokeCommand("unsubscribe_harness_graph_tools", { subscriptionId }).catch(() => {});
    });
    channel.onmessage = (value) => {
      try {
        const request = parseHarnessGraphToolRequest(value);
        if (!closed && request.sessionId === sessionId)
          void onRequest(request).catch(() =>
            HarnessGraphToolsService.complete(request.requestId, false).catch(() => {}),
          );
      } catch {
        void invokeCommand("unsubscribe_harness_graph_tools", { subscriptionId }).catch(() => {});
      }
    };
    try {
      const result = await invokeCommand("subscribe_harness_graph_tools", {
        sessionId,
        onRequest: channel,
      });
      if (typeof result !== "string" || !result) throw new Error("Invalid graph tool subscription");
      subscriptionId = result;
      if (closed) await invokeCommand("unsubscribe_harness_graph_tools", { subscriptionId });
    } catch (error) {
      cleanup();
      throw error;
    }
    return {
      close: async () => {
        if (closed) return;
        cleanup();
        if (subscriptionId)
          await invokeCommand("unsubscribe_harness_graph_tools", { subscriptionId });
      },
    };
  }

  static async prepare(
    request: HarnessGraphToolRequest,
    document: GraphDocumentDto,
    draftGeneration: number,
    locale: string,
    onEvent: (event: RunEvent) => void,
    onOutput: (event: RunOutputChannelEvent) => void,
  ): Promise<HarnessGraphUpdate> {
    const drain = createExecutionStreamDrain(onEvent, onOutput);
    const channel = trackChannel(new Channel<unknown>(), drain.dispose);
    channel.onmessage = drain.onmessage;
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      const update = parseUpdate(
        await invokeCommand("prepare_harness_graph_tool", {
          input: {
            requestId: request.requestId,
            projectInstanceId: request.projectInstanceId,
            document,
            draftGeneration,
            locale,
          },
          onExecutionEvent: channel,
        }),
        request.projectInstanceId,
      );
      const returnedPath =
        update.type === "draft" || update.type === "compilation"
          ? update.update.projection.graphPath
          : update.type === "saved"
            ? update.update.projectionReplacement.graphPath
            : request.graphPath;
      if (returnedPath !== request.graphPath)
        throw new Error("Graph tool response targets another graph");
      if (update.type === "execution" && update.update.terminalEventSent) {
        await Promise.race([
          drain.waitForStreamEnd(),
          new Promise<never>((_, reject) => {
            timer = setTimeout(() => reject(new Error("Execution event delivery timed out")), 5000);
          }),
        ]);
      }
      return update;
    } finally {
      if (timer) clearTimeout(timer);
      drain.dispose();
      untrackChannel(channel);
      clearChannelMessageHandler(channel);
    }
  }

  static async complete(requestId: string, applied: boolean): Promise<void> {
    await invokeCommand("complete_harness_graph_tool", { requestId, applied });
  }
  static async claim(requestId: string): Promise<boolean> {
    return (await invokeCommand("claim_harness_graph_tool", { requestId })) === true;
  }
}
