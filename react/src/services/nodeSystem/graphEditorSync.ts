import { z } from "zod";
import { freezePublishedValue } from "@/shared/types/deepReadonly";
import { validateEditorGraphProjection } from "@/shared/types/domain/editorProjectionParser";
import { applyGraphChanges, graphChangesSchema } from "./graphEditorChanges";
import { invokeCommand, isIpcError } from "@/services/ipc";
import {
  parseGraphEditorSessionDto,
  parseGraphEditVersion,
} from "@/shared/types/dto/editorMutationWireParser";
import type { GraphEditorSessionDto } from "@/shared/types/domain/editorMutation";

export interface GraphSyncBinding {
  projectInstanceId: string;
  graphPath: string;
  locale: string;
}
interface Baseline {
  cursor: string;
  data: GraphEditorSessionDto;
  bytes: number;
}
export interface GraphSyncReply {
  data: GraphEditorSessionDto;
  changed: boolean;
  resourceRevision: number | null;
  functionEditorProjection: unknown;
}
const baselines = new Map<string, Baseline>();
let epoch = 0;
const key = (binding: GraphSyncBinding) =>
  JSON.stringify([binding.projectInstanceId, binding.graphPath, binding.locale]);
const requiredPayload = z.unknown().refine((value) => value !== undefined);
const cursor = z.string().min(1);
const integer = z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER);
const responseSchema = z.strictObject({
  projectInstanceId: z.string(),
  graphPath: z.string(),
  locale: z.string(),
  changed: z.boolean(),
  resourceRevision: integer.nullable(),
  functionEditorProjection: requiredPayload,
  update: requiredPayload,
});
const deliverySchema = z.discriminatedUnion("kind", [
  z.strictObject({
    kind: z.literal("snapshot"),
    cursor,
    snapshotBytes: integer.positive(),
    data: requiredPayload,
  }),
  z.strictObject({
    kind: z.literal("delta"),
    baseCursor: cursor,
    cursor,
    snapshotBytes: integer.positive(),
    changes: graphChangesSchema,
  }),
]);
const receiptSchema = z.strictObject({
  projectInstanceId: z.string(),
  graphPath: z.string(),
  operationId: z.string(),
  requestVersion: requiredPayload,
  committedVersion: requiredPayload,
  command: z.enum(["edit", "save", "undo", "redo"]),
  changed: z.boolean(),
});
type GraphResponse = z.infer<typeof responseSchema>;

function envelope(value: unknown, binding: GraphSyncBinding): GraphResponse {
  const parsed = responseSchema.safeParse(value);
  if (
    !parsed.success ||
    parsed.data.projectInstanceId !== binding.projectInstanceId ||
    parsed.data.graphPath !== binding.graphPath ||
    parsed.data.locale !== binding.locale
  ) {
    throw new Error("Graph response binding is malformed");
  }
  return parsed.data;
}

function decode(value: GraphResponse, previous: Baseline | undefined): Baseline {
  const parsed = deliverySchema.safeParse(value.update);
  if (!parsed.success) throw new Error("Graph delivery is malformed");
  const update = parsed.data;
  let candidate: unknown;
  if (update.kind === "snapshot") candidate = update.data;
  else if (previous && update.baseCursor === previous.cursor)
    candidate = applyGraphChanges(previous.data, update.changes);
  else throw new Error("Graph delivery baseline is unavailable");
  // Freeze the owned candidate before parsing so successful checks can be cached on the
  // first pass. Neither the baseline nor the store sees it until validation has succeeded.
  freezePublishedValue(candidate);
  const data = parseGraphEditorSessionDto(candidate);
  if (data.projection.graphPath !== value.graphPath)
    throw new Error("Graph projection identity mismatch");
  validateEditorGraphProjection(data.projection);
  if (
    update.kind === "delta" &&
    previous &&
    (data.editing.version.sessionId !== previous.data.editing.version.sessionId ||
      BigInt(data.editing.version.revision) < BigInt(previous.data.editing.version.revision))
  )
    throw new Error("Graph delivery editing session mismatch");
  // Parsing creates a session/editing envelope; its DTO branches are already frozen.
  freezePublishedValue(data);
  return { cursor: update.cursor, data, bytes: update.snapshotBytes };
}

type GraphSyncCommand =
  | "load_project_graph"
  | "hydrate_editor_graph"
  | "resolve_editor_graph"
  | "edit_graph"
  | "save_project_graph"
  | "change_graph_history";

function invokeBound(
  command: GraphSyncCommand,
  args: Record<string, unknown>,
  binding: GraphSyncBinding,
  cursor: string | null,
): Promise<unknown> {
  const { projectInstanceId, graphPath, locale } = binding;
  switch (command) {
    case "load_project_graph":
      return invokeCommand("load_project_graph", {
        ...args,
        projectInstanceId,
        graphPath,
        locale,
        cursor,
      });
    case "hydrate_editor_graph":
      return invokeCommand("hydrate_editor_graph", {
        ...args,
        projectInstanceId,
        graphPath,
        locale,
        cursor,
      });
    case "resolve_editor_graph":
      return invokeCommand("resolve_editor_graph", {
        ...args,
        projectInstanceId,
        graphPath,
        locale,
        cursor,
      });
    case "edit_graph":
      return invokeCommand("edit_graph", { ...args, projectInstanceId, graphPath, locale, cursor });
    case "save_project_graph":
      return invokeCommand("save_project_graph", {
        ...args,
        projectInstanceId,
        graphPath,
        locale,
        cursor,
      });
    case "change_graph_history":
      return invokeCommand("change_graph_history", {
        ...args,
        projectInstanceId,
        graphPath,
        locale,
        cursor,
      });
  }
}

export async function invokeGraphSync(
  command: GraphSyncCommand,
  args: Record<string, unknown>,
  binding: GraphSyncBinding,
): Promise<GraphSyncReply> {
  const requestEpoch = epoch;
  const bindingKey = key(binding);
  const previous = baselines.get(bindingKey);
  let response: GraphResponse;
  try {
    response = envelope(
      await invokeBound(command, args, binding, previous?.cursor ?? null),
      binding,
    );
  } catch (error) {
    // A business rejection is final. An absent/malformed reply may follow a real commit.
    if (isIpcError(error) && error.kind === "backend" && error.code !== "internal_error")
      throw error;
    response = await recoverCommittedCommand(command, args, binding, error);
  }
  let decoded: Baseline;
  try {
    decoded = decode(response, previous);
  } catch {
    // Recover only the read projection. A mutation whose reply was lost must never be replayed.
    const recovered = envelope(
      await invokeBound("hydrate_editor_graph", {}, binding, null),
      binding,
    );
    decoded = decode(recovered, undefined);
    response.functionEditorProjection = recovered.functionEditorProjection;
  }
  if (requestEpoch !== epoch) throw new Error("Graph response belongs to a released view");
  baselines.delete(bindingKey);
  if (decoded.bytes <= 16 * 1024 * 1024) baselines.set(bindingKey, decoded);
  let retainedBytes = [...baselines.values()].reduce((total, entry) => total + entry.bytes, 0);
  while (baselines.size > 32 || retainedBytes > 32 * 1024 * 1024) {
    const oldest = baselines.keys().next().value!;
    retainedBytes -= baselines.get(oldest)!.bytes;
    baselines.delete(oldest);
  }
  return {
    data: decoded.data,
    changed: response.changed,
    resourceRevision: response.resourceRevision,
    functionEditorProjection: response.functionEditorProjection,
  };
}

async function recoverCommittedCommand(
  command: GraphSyncCommand,
  args: Record<string, unknown>,
  binding: GraphSyncBinding,
  originalError: unknown,
): Promise<GraphResponse> {
  const kind =
    command === "edit_graph"
      ? "edit"
      : command === "save_project_graph"
        ? "save"
        : command === "change_graph_history"
          ? args.redo
            ? "redo"
            : "undo"
          : null;
  if (!kind || typeof args.operationId !== "string") throw originalError;
  try {
    const version = parseGraphEditVersion(args.version);
    const result = receiptSchema.safeParse(
      await invokeCommand<unknown>("get_graph_edit_receipt", {
        projectInstanceId: binding.projectInstanceId,
        graphPath: binding.graphPath,
        version,
        operationId: args.operationId,
      }),
    );
    if (!result.success) throw originalError;
    const receipt = result.data;
    if (
      receipt.projectInstanceId !== binding.projectInstanceId ||
      receipt.graphPath !== binding.graphPath ||
      receipt.operationId !== args.operationId ||
      receipt.command !== kind
    )
      throw originalError;
    const requested = parseGraphEditVersion(receipt.requestVersion);
    const committed = parseGraphEditVersion(receipt.committedVersion);
    if (
      requested.sessionId !== version.sessionId ||
      requested.revision !== version.revision ||
      committed.sessionId !== version.sessionId ||
      BigInt(committed.revision) <= BigInt(version.revision)
    )
      throw originalError;
    const recovered = envelope(
      await invokeBound("hydrate_editor_graph", {}, binding, null),
      binding,
    );
    const current = decode(recovered, undefined).data.editing.version;
    if (
      current.sessionId !== committed.sessionId ||
      BigInt(current.revision) < BigInt(committed.revision)
    )
      throw originalError;
    recovered.changed = receipt.changed;
    recovered.resourceRevision = kind === "save" ? Number(committed.revision) : null;
    return envelope(recovered, binding);
  } catch {
    // Unknown/expired receipts do not authorize retrying the write or claiming success.
    throw originalError;
  }
}

export function clearGraphSyncBaselines(): void {
  epoch++;
  baselines.clear();
}
