import { produce } from "immer";
import { z } from "zod";

const path = z.array(z.string()).max(32);
const payload = z.unknown().refine((value) => value !== undefined);
const index = z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER);

// This is the Rust GraphProjectionChangeDto protocol, not RFC 6902 or Immer's patch wire.
export const graphChangesSchema = z
  .array(
    z.discriminatedUnion("kind", [
      z.strictObject({ kind: z.literal("set"), path, value: payload }),
      z.strictObject({ kind: z.literal("remove"), path: path.min(1) }),
      z.strictObject({
        kind: z.literal("splice"),
        path,
        index,
        deleteCount: index.max(512),
        values: z.array(payload).max(512),
      }),
    ]),
  )
  .max(512);

type GraphChange = z.infer<typeof graphChangesSchema>[number];
type Container = Record<string, unknown> | unknown[];
const own = (value: object, key: string) => Object.prototype.hasOwnProperty.call(value, key);

function container(value: unknown): Container {
  if (!value || typeof value !== "object") throw new Error("Graph patch parent is not a container");
  return value as Container;
}

function property(parent: Container, part: string): string {
  if (part === "__proto__") throw new Error("Unsafe graph patch property");
  if (Array.isArray(parent) && (!/^(0|[1-9][0-9]*)$/u.test(part) || Number(part) >= parent.length))
    throw new Error("Invalid graph array index");
  return part;
}

function read(root: unknown, path: readonly string[]): unknown {
  let value = root;
  for (const segment of path) {
    const parent = container(value);
    const part = property(parent, segment);
    if (!own(parent, part)) throw new Error("Missing graph patch parent");
    value = (parent as Record<string, unknown>)[part];
  }
  return value;
}

export function applyGraphChanges(previous: unknown, changes: readonly GraphChange[]): unknown {
  // The holder permits root replacement within the same atomic batch. Incoming values
  // are detached because Immer does not draft newly assigned objects on later operations.
  return produce({ data: previous }, (draft) => {
    for (const operation of changes) {
      if (operation.kind === "splice") {
        const target = read(draft.data, operation.path);
        if (
          !Array.isArray(target) ||
          operation.index > target.length ||
          operation.deleteCount > target.length - operation.index
        )
          throw new Error("Invalid graph array splice");
        target.splice(operation.index, operation.deleteCount, ...structuredClone(operation.values));
        continue;
      }
      if (operation.path.length === 0) {
        if (operation.kind !== "set") throw new Error("Cannot remove the graph projection root");
        draft.data = structuredClone(operation.value);
        continue;
      }
      const parent = container(read(draft.data, operation.path.slice(0, -1)));
      const part = property(parent, operation.path[operation.path.length - 1]);
      if (operation.kind === "remove") {
        if (Array.isArray(parent) || !own(parent, part)) throw new Error("Invalid graph removal");
        delete parent[part];
      } else {
        (parent as Record<string, unknown>)[part] = structuredClone(operation.value);
      }
    }
  }).data;
}
