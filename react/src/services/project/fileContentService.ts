import { invokeCommand } from "@/services/ipc";
import { parseResourceMutationResultDto } from "@/shared/types/dto/resourceMutationResultWireParser";
import type { FileSnapshot, FileCommand } from "@/shared/types/domain/fileDocument";
import type { FileResourceKind } from "@/shared/types/domain/resource";
export function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
export function exact(
  value: Record<string, unknown>,
  required: string[],
  optional: string[] = [],
): boolean {
  return (
    required.every((key) => Object.prototype.hasOwnProperty.call(value, key)) &&
    Object.keys(value).every((key) => required.includes(key) || optional.includes(key))
  );
}
export function parseFileSnapshot<K extends FileResourceKind, C>(
  value: unknown,
  kind: K,
  isPath: (value: unknown) => value is string,
  parseContent: (value: unknown) => C,
): FileSnapshot<K, C> {
  if (
    !record(value) ||
    !exact(value, ["projectInstanceId", "path", "version", "kind", "content", "dirty"]) ||
    value.kind !== kind ||
    !isPath(value.path) ||
    typeof value.projectInstanceId !== "string" ||
    !value.projectInstanceId ||
    !record(value.version) ||
    !exact(value.version, ["sessionId", "revision"]) ||
    typeof value.version.sessionId !== "string" ||
    !value.version.sessionId ||
    !Number.isSafeInteger(value.version.revision) ||
    (value.version.revision as number) < 0 ||
    typeof value.dirty !== "boolean"
  )
    throw new Error("Invalid file snapshot");
  return {
    projectInstanceId: value.projectInstanceId,
    path: value.path,
    kind,
    content: parseContent(value.content),
    dirty: value.dirty,
    version: { sessionId: value.version.sessionId, revision: value.version.revision as number },
  };
}
export function createFileContentService<S, E>(
  readCommand: string,
  editCommand: string,
  parse: (value: unknown) => S,
) {
  return {
    async read(projectInstanceId: string, path: string): Promise<S> {
      return parse(await invokeCommand<unknown>(readCommand, { projectInstanceId, path }));
    },
    async command(projectInstanceId: string, operationId: string, command: FileCommand<E>) {
      const value = await invokeCommand<unknown>(editCommand, {
        projectInstanceId,
        operationId,
        command,
      });
      if (!record(value) || !exact(value, ["snapshot", "mutation"]))
        throw new Error("Invalid file command result");
      return {
        snapshot: value.snapshot === null ? null : parse(value.snapshot),
        mutation: parseResourceMutationResultDto(value.mutation),
      };
    },
  };
}
