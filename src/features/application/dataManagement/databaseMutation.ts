import { useDatabaseStore } from "@/features/core/dataStore/databaseStore";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import {
  captureProjectCommandContext,
  captureRevisionedProjectCommandSnapshot,
} from "@/features/application/projectCommandContext";
import type { DatabaseMutationCommandResult } from "@/services/database/databaseService";
import { DatabaseService } from "@/services/database/databaseService";
import type { ColumnSemantic } from "@/shared/types/domain/database";

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

export async function executeDatabaseCreate<T>(
  command: (authority: DatabaseCommandAuthority) => Promise<DatabaseMutationCommandResult<T>>,
): Promise<T> {
  const context = captureProjectCommandContext();
  const aggregate = await command(context);
  return settle(context, aggregate);
}

export async function executeDatabaseMutation<T>(
  id: string,
  command: (
    authority: RevisionedDatabaseCommandAuthority,
  ) => Promise<DatabaseMutationCommandResult<T>>,
): Promise<T> {
  const { context, captured: expectedRevision } = captureRevisionedProjectCommandSnapshot(
    () => useDatabaseStore.getState().revisions[id],
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
