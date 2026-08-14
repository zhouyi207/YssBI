import type { ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';

export interface WorksheetCommandContext {
  projectInstanceId: string;
  operationId: string;
  isCurrent(): boolean;
}

export interface WorksheetApplicationPort {
  captureCommandContext(): WorksheetCommandContext;
  submitPublication(result: ResourceMutationResultDto): Promise<unknown>;
}

let port: WorksheetApplicationPort | null = null;

export function registerWorksheetApplicationPort(next: WorksheetApplicationPort): void {
  port = next;
}

export function resetWorksheetApplicationPort(): void {
  port = null;
}

export function worksheetApplicationPort(): WorksheetApplicationPort {
  if (!port) throw new Error('Worksheet application port is not registered');
  return port;
}
