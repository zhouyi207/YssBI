import { useResourceStore } from "@/features/core/resource/resourceStore";
import { resourceKey } from "@/features/core/resource/resourceTypes";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import {
  captureProjectCommandContext,
  captureRevisionedProjectCommandSnapshot,
} from "@/features/application/projectCommandContext";
import type { DatabaseMutationCommandResult } from "@/services/database/databaseService";
import { DatabaseService } from "@/services/database/databaseService";
import type { ColumnSemantic, LoadDatabaseResult } from "@/shared/types/domain/database";

interface DatabaseCommandAuthority {
  projectInstanceId: string;
  operationId: string;
}

interface RevisionedDatabaseCommandAuthority extends DatabaseCommandAuthority {
  expectedRevision: number;
}

async function settle<T>(
  context: ReturnType<typeof captureProjectCommandContext>,
  aggregate: DatabaseMutationCommandResult<T>,
): Promise<T> {
  context.assertCurrent();
  if (
    aggregate.mutation.projectInstanceId !== context.projectInstanceId ||
    aggregate.mutation.operationId !== context.operationId
  ) {
    throw new Error("database mutation receipt correlation is invalid");
  }
  await projectPublicationCoordinator.submit({ result: aggregate.mutation });
  context.assertCurrent();
  return aggregate.data;
}

export async function executeDatabaseCreate(
  command: (
    authority: DatabaseCommandAuthority,
  ) => Promise<DatabaseMutationCommandResult<LoadDatabaseResult>>,
): Promise<void> {
  const context = captureProjectCommandContext();
  const aggregate = await command(context);
  context.assertCurrent();
  const { data, mutation } = aggregate;
  const created = mutation.deltas.find(
    (delta) =>
      delta.resource.kind === "database" &&
      delta.payload.kind === "database" &&
      delta.payload.patch.before === null &&
      delta.payload.patch.after?.id === data.id,
  );
  if (!created) throw new Error("database import receipt has no matching creation");
  await settle(context, aggregate);
  context.assertCurrent();
  const current = useResourceStore.getState();
  if (current.databases[data.id]?.resourcePath !== created.resource.key) return;
  current.updateDatabaseMetadata(data.id, created.toRevision, data);
}

export async function executeDatabaseMutation<T>(
  id: string,
  command: (
    authority: RevisionedDatabaseCommandAuthority,
  ) => Promise<DatabaseMutationCommandResult<T>>,
): Promise<T> {
  const { context, captured: expectedRevision } = captureRevisionedProjectCommandSnapshot(
    () => useResourceStore.getState().resources[resourceKey({ kind: "database", id })]?.revision,
  );
  if (expectedRevision == null) {
    throw new Error(`Database '${id}' has no authoritative revision`);
  }
  const aggregate = await command({
    projectInstanceId: context.projectInstanceId,
    operationId: context.operationId,
    expectedRevision,
  });
  return settle(context, aggregate);
}

export function changeColumnPhysical(id: string, column: string, physical: string) {
  return executeDatabaseMutation(id, (authority) =>
    DatabaseService.castColumn(
      authority.projectInstanceId,
      authority.operationId,
      authority.expectedRevision,
      id,
      column,
      physical,
    ),
  );
}

export function changeColumnSemantic(id: string, column: string, semantic: ColumnSemantic) {
  return executeDatabaseMutation(id, (authority) =>
    DatabaseService.setColumnSemantic(
      authority.projectInstanceId,
      authority.operationId,
      authority.expectedRevision,
      id,
      column,
      semantic,
    ),
  );
}
