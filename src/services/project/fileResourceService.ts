import { invokeCommand } from "@/services/ipc";
import { parseResourceMutationResultDto } from "@/shared/types/dto/resourceMutationResultWireParser";

/** Transport mechanics shared by independently registered Event and Function commands. */
function fileService(commands: {
  create: string;
  rename: string;
  duplicate: string;
  remove: string;
}) {
  const invoke = async (command: string, args: Record<string, unknown>) =>
    parseResourceMutationResultDto(await invokeCommand<unknown>(command, args));
  return {
    create: (projectInstanceId: string, operationId: string, name: string) =>
      invoke(commands.create, { projectInstanceId, operationId, name }),
    rename: (
      projectInstanceId: string,
      operationId: string,
      path: string,
      expectedRevision: number,
      name: string,
      lifecycleToken: number,
    ) =>
      invoke(commands.rename, {
        projectInstanceId,
        operationId,
        path,
        expectedRevision,
        name,
        lifecycleToken,
      }),
    duplicate: (
      projectInstanceId: string,
      operationId: string,
      path: string,
      expectedRevision: number,
    ) => invoke(commands.duplicate, { projectInstanceId, operationId, path, expectedRevision }),
    remove: (
      projectInstanceId: string,
      operationId: string,
      path: string,
      expectedRevision: number,
    ) => invoke(commands.remove, { projectInstanceId, operationId, path, expectedRevision }),
  };
}

export const EventGraphService = fileService({
  create: "create_event_graph",
  rename: "rename_event_graph",
  duplicate: "duplicate_event_graph",
  remove: "remove_event_graph",
});
export const FunctionGraphService = fileService({
  create: "create_function_graph",
  rename: "rename_function_graph",
  duplicate: "duplicate_function_graph",
  remove: "remove_function_graph",
});
